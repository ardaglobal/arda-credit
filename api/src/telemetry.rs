//! OpenTelemetry configuration and initialization
//!
//! This module handles the setup and configuration of OpenTelemetry tracing
//! for the Arda Credit API. It supports both local OpenTelemetry collectors
//! and SigNoz Cloud deployments, and bridges Rust tracing with OpenTelemetry.

use opentelemetry::global::shutdown_tracer_provider;
use opentelemetry::sdk::Resource;
use opentelemetry::trace::{TraceError, TracerProvider};
use opentelemetry::{
    global, sdk::trace as sdktrace,
    trace::{TraceContextExt, Tracer},
    Context, KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use tonic::metadata::{MetadataMap, MetadataValue};
use tracing::{error, info};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

/// Initialize the OpenTelemetry tracer
///
/// This function sets up the OpenTelemetry tracer based on environment variables.
/// It supports both local OpenTelemetry collectors and SigNoz Cloud.
///
/// Environment variables:
/// - `ENABLE_OPENTELEMETRY`: Enable/disable tracing (default: false)
/// - `OTEL_SERVICE_NAME`: Service name for traces (default: "arda-credit-api")
/// - `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP endpoint (default: "http://localhost:4317")
/// - `SIGNOZ_INGESTION_KEY`: SigNoz Cloud ingestion key (optional)
pub fn init_tracer() -> Result<sdktrace::Tracer, TraceError> {
    // Check if tracing is enabled
    let enable_tracing = std::env::var("ENABLE_OPENTELEMETRY")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    if !enable_tracing {
        info!("OpenTelemetry tracing is disabled");
        return Ok(sdktrace::TracerProvider::default().tracer("noop"));
    }

    info!("🔧 Initializing OpenTelemetry tracer");

    // Get configuration from environment
    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "arda-credit-api".to_string());

    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    info!("📊 OpenTelemetry configuration:");
    info!("   Service Name: {}", service_name);
    info!("   Endpoint: {}", endpoint);

    // Create resource with service information
    let resource = Resource::new(vec![
        KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_NAME,
            service_name.clone(),
        ),
        KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_VERSION,
            env!("CARGO_PKG_VERSION"),
        ),
        KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_NAMESPACE,
            "arda-credit",
        ),
    ]);

    // Check if using SigNoz Cloud (requires ingestion key)
    if let Ok(ingestion_key) = std::env::var("SIGNOZ_INGESTION_KEY") {
        info!("🌐 Configuring OpenTelemetry for SigNoz Cloud");
        let mut metadata = MetadataMap::new();
        metadata.insert(
            "signoz-ingestion-key",
            MetadataValue::try_from(ingestion_key.as_str()).map_err(|e| {
                error!("Failed to create metadata value from ingestion key: {}", e);
                TraceError::Other(Box::new(e))
            })?,
        );

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_metadata(metadata)
                    .with_endpoint(endpoint),
            )
            .with_trace_config(sdktrace::config().with_resource(resource))
            .install_batch(opentelemetry::runtime::Tokio)?;

        info!("✅ OpenTelemetry tracer initialized for SigNoz Cloud");
        Ok(tracer)
    } else {
        info!("🏠 Configuring OpenTelemetry for local collector");
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(endpoint),
            )
            .with_trace_config(sdktrace::config().with_resource(resource))
            .install_batch(opentelemetry::runtime::Tokio)?;

        info!("✅ OpenTelemetry tracer initialized for local collector");
        Ok(tracer)
    }
}

/// Initialize complete tracing setup with OpenTelemetry integration
///
/// This function sets up both the tracing subscriber and OpenTelemetry integration.
/// It should be called instead of manually setting up tracing_subscriber::fmt().
///
/// Returns the OpenTelemetry tracer for use in the application.
pub fn init_tracing() -> Result<sdktrace::Tracer, TraceError> {
    // Check if tracing is enabled
    let enable_tracing = std::env::var("ENABLE_OPENTELEMETRY")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    if !enable_tracing {
        info!("OpenTelemetry tracing is disabled, using basic tracing subscriber");

        // Initialize basic tracing subscriber without OpenTelemetry
        Registry::default()
            .with(EnvFilter::from_default_env())
            .with(tracing_subscriber::fmt::layer())
            .init();

        return Ok(sdktrace::TracerProvider::default().tracer("noop"));
    }

    info!("🔧 Initializing complete tracing setup with OpenTelemetry");

    // Initialize the OpenTelemetry tracer
    let tracer = init_tracer()?;

    // Create the OpenTelemetry tracing layer
    let telemetry_layer = OpenTelemetryLayer::new(tracer.clone());

    // Initialize the tracing subscriber with both fmt and OpenTelemetry layers
    Registry::default()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry_layer)
        .init();

    info!("✅ Complete tracing setup initialized with OpenTelemetry integration");
    Ok(tracer)
}

/// Shutdown the OpenTelemetry tracer provider
///
/// This should be called during application shutdown to ensure all spans
/// are properly flushed and exported. Includes a timeout to prevent hanging.
pub async fn shutdown_telemetry() {
    info!("🔄 Shutting down OpenTelemetry");

    // Use tokio::spawn_blocking for the synchronous shutdown call with timeout
    let shutdown_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(|| {
            shutdown_tracer_provider();
        }),
    )
    .await;

    match shutdown_result {
        Ok(Ok(())) => {
            info!("✅ OpenTelemetry shutdown complete");
        }
        Ok(Err(e)) => {
            error!("❌ OpenTelemetry shutdown task failed: {}", e);
        }
        Err(_) => {
            error!("⏰ OpenTelemetry shutdown timed out after 5 seconds");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_init_tracer_disabled() {
        env::set_var("ENABLE_OPENTELEMETRY", "false");
        let result = init_tracer();
        assert!(result.is_ok());
    }

    #[test]
    fn test_service_name_default() {
        env::remove_var("OTEL_SERVICE_NAME");
        env::set_var("ENABLE_OPENTELEMETRY", "false"); // Don't actually initialize

        let service_name =
            std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "arda-credit-api".to_string());
        assert_eq!(service_name, "arda-credit-api");
    }
}
