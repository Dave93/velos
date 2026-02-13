use opentelemetry::trace::{TraceContextExt, Tracer, TracerProvider};
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use velos_core::VelosError;

/// Initialize an OpenTelemetry TracerProvider with OTLP HTTP exporter.
///
/// Returns the provider so callers can create tracers and spans.
/// The provider must be kept alive for the lifetime of the application.
pub fn init_tracer_provider(endpoint: &str) -> Result<SdkTracerProvider, VelosError> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| VelosError::ProtocolError(format!("otel exporter init: {e}")))?;

    let hostname = hostname();

    let resource = Resource::builder()
        .with_attribute(KeyValue::new("service.name", "velos"))
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .with_attribute(KeyValue::new("host.name", hostname))
        .build();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    Ok(provider)
}

/// Record a process lifecycle event as a span.
pub fn record_lifecycle_event(
    provider: &SdkTracerProvider,
    event: &str,
    process_name: &str,
    process_id: u32,
) {
    let tracer = provider.tracer("velos");
    tracer.in_span(format!("process.{event}"), |cx| {
        let span = cx.span();
        span.set_attribute(KeyValue::new("process.name", process_name.to_string()));
        span.set_attribute(KeyValue::new("process.id", process_id as i64));
    });
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".to_string())
}
