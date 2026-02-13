use std::time::Duration;
use velos_core::VelosError;

/// Start the Prometheus metrics server (and optionally init OTel tracing).
pub async fn run(port: u16, otel_endpoint: Option<String>) -> Result<(), VelosError> {
    // Optionally initialise OpenTelemetry
    let _provider = if let Some(ref ep) = otel_endpoint {
        let p = velos_metrics::otel::init_tracer_provider(ep)?;
        println!("OpenTelemetry exporter configured → {ep}");
        Some(p)
    } else {
        None
    };

    // Start Prometheus HTTP server (blocking)
    velos_metrics::prometheus::serve(port, Duration::from_secs(5)).await
}
