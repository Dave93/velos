use std::collections::VecDeque;

/// Severity of an anomaly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum AnomalySeverity {
    Warning,
    Critical,
}

impl AnomalySeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

/// A detected anomaly in log metrics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Anomaly {
    pub metric: String,
    pub current_value: f64,
    pub mean: f64,
    pub std_dev: f64,
    pub sigma: f64,
    pub timestamp_ms: u64,
    pub severity: AnomalySeverity,
}

/// Generic sliding window for time-series metrics.
#[derive(Debug, Clone)]
pub struct SlidingWindow {
    values: VecDeque<f64>,
    capacity: usize,
}

impl SlidingWindow {
    pub fn new(capacity: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: f64) {
        if self.values.len() >= self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn mean(&self) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.values.iter().sum();
        sum / self.values.len() as f64
    }

    pub fn std_dev(&self) -> f64 {
        if self.values.len() < 2 {
            return 0.0;
        }
        let mean = self.mean();
        let variance: f64 = self.values.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
            / (self.values.len() - 1) as f64;
        variance.sqrt()
    }
}

/// Anomaly detector using sliding windows for error_rate and log_volume.
pub struct AnomalyDetector {
    pub error_rate: SlidingWindow,
    pub log_volume: SlidingWindow,
    window_size: usize,
    sigma_warning: f64,
    sigma_critical: f64,
    min_data_points: usize,
}

impl AnomalyDetector {
    pub fn new(window_size: usize, sigma_warning: f64, sigma_critical: f64) -> Self {
        Self {
            error_rate: SlidingWindow::new(window_size),
            log_volume: SlidingWindow::new(window_size),
            window_size,
            sigma_warning,
            sigma_critical,
            min_data_points: 10,
        }
    }

    /// Default: 60-minute window, 2σ warning, 3σ critical.
    pub fn with_defaults() -> Self {
        Self::new(60, 2.0, 3.0)
    }

    /// Record a data point (call once per minute).
    pub fn record(&mut self, errors_per_minute: f64, lines_per_minute: f64) {
        self.error_rate.push(errors_per_minute);
        self.log_volume.push(lines_per_minute);
    }

    /// Check for anomalies against current values.
    pub fn check(&self, current_errors: f64, current_volume: f64, now_ms: u64) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        if self.error_rate.len() < self.min_data_points {
            return anomalies;
        }

        // Check error_rate
        if let Some(a) = self.check_metric("error_rate", &self.error_rate, current_errors, now_ms) {
            anomalies.push(a);
        }

        // Check log_volume
        if let Some(a) = self.check_metric("log_volume", &self.log_volume, current_volume, now_ms) {
            anomalies.push(a);
        }

        anomalies
    }

    fn check_metric(
        &self,
        name: &str,
        window: &SlidingWindow,
        current: f64,
        now_ms: u64,
    ) -> Option<Anomaly> {
        let mean = window.mean();
        let std_dev = window.std_dev();

        if std_dev < f64::EPSILON {
            // No variance — can't detect anomaly
            return None;
        }

        let sigma = (current - mean) / std_dev;

        if sigma >= self.sigma_critical {
            Some(Anomaly {
                metric: name.to_string(),
                current_value: current,
                mean,
                std_dev,
                sigma,
                timestamp_ms: now_ms,
                severity: AnomalySeverity::Critical,
            })
        } else if sigma >= self.sigma_warning {
            Some(Anomaly {
                metric: name.to_string(),
                current_value: current,
                mean,
                std_dev,
                sigma,
                timestamp_ms: now_ms,
                severity: AnomalySeverity::Warning,
            })
        } else {
            None
        }
    }

    /// Whether enough data has been accumulated.
    pub fn has_enough_data(&self) -> bool {
        self.error_rate.len() >= self.min_data_points
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }
}

/// Compute error_rate and log_volume from a batch of entries within a time bucket.
pub fn compute_minute_metrics(
    entries: &[crate::ProcessedEntry],
    bucket_start_ms: u64,
    bucket_end_ms: u64,
) -> (f64, f64) {
    let mut errors = 0u32;
    let mut total = 0u32;

    for e in entries {
        if e.timestamp_ms >= bucket_start_ms && e.timestamp_ms < bucket_end_ms {
            total += 1;
            if matches!(e.level, crate::LogLevel::Error | crate::LogLevel::Fatal) {
                errors += 1;
            }
        }
    }

    (errors as f64, total as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sliding_window_basic() {
        let mut w = SlidingWindow::new(5);
        for i in 1..=5 {
            w.push(i as f64);
        }
        assert_eq!(w.len(), 5);
        assert!((w.mean() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sliding_window_overflow() {
        let mut w = SlidingWindow::new(3);
        w.push(1.0);
        w.push(2.0);
        w.push(3.0);
        w.push(4.0); // evicts 1.0
        assert_eq!(w.len(), 3);
        assert!((w.mean() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_sliding_window_std_dev() {
        let mut w = SlidingWindow::new(10);
        // All same value → std_dev = 0
        for _ in 0..10 {
            w.push(5.0);
        }
        assert!(w.std_dev() < f64::EPSILON);
    }

    #[test]
    fn test_anomaly_detection_no_data() {
        let detector = AnomalyDetector::with_defaults();
        let anomalies = detector.check(100.0, 1000.0, 0);
        assert!(anomalies.is_empty()); // Not enough data
    }

    #[test]
    fn test_anomaly_detection_normal() {
        let mut detector = AnomalyDetector::with_defaults();
        // Feed normal data: ~5 errors/min
        for _ in 0..20 {
            detector.record(5.0, 100.0);
        }
        // Check with a normal value
        let anomalies = detector.check(6.0, 110.0, 1000);
        assert!(anomalies.is_empty());
    }

    #[test]
    fn test_anomaly_detection_spike() {
        let mut detector = AnomalyDetector::new(60, 2.0, 3.0);
        // Feed stable data: 5 errors/min, slight variance
        for i in 0..30 {
            detector.record(5.0 + (i % 3) as f64 * 0.5, 100.0);
        }
        // Huge spike: 50 errors/min
        let anomalies = detector.check(50.0, 100.0, 1000);
        assert!(!anomalies.is_empty());
        assert_eq!(anomalies[0].metric, "error_rate");
        assert!(anomalies[0].sigma >= 2.0);
    }

    #[test]
    fn test_anomaly_severity_levels() {
        let mut detector = AnomalyDetector::new(60, 2.0, 3.0);
        for _ in 0..30 {
            detector.record(5.0, 100.0);
        }
        // Add slight variance
        detector.record(6.0, 105.0);

        // Very large spike → critical
        let anomalies = detector.check(500.0, 100.0, 1000);
        assert!(!anomalies.is_empty());
        assert_eq!(anomalies[0].severity, AnomalySeverity::Critical);
    }

    #[test]
    fn test_compute_minute_metrics() {
        let entries = vec![
            crate::ProcessedEntry {
                timestamp_ms: 1000,
                level: crate::LogLevel::Info,
                stream: 0,
                message: "ok".into(),
            },
            crate::ProcessedEntry {
                timestamp_ms: 2000,
                level: crate::LogLevel::Error,
                stream: 0,
                message: "fail".into(),
            },
            crate::ProcessedEntry {
                timestamp_ms: 3000,
                level: crate::LogLevel::Info,
                stream: 0,
                message: "ok2".into(),
            },
        ];
        let (errors, total) = compute_minute_metrics(&entries, 0, 60000);
        assert!((errors - 1.0).abs() < f64::EPSILON);
        assert!((total - 3.0).abs() < f64::EPSILON);
    }
}
