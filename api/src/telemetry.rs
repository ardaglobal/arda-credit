//! OpenTelemetry configuration and initialization
//!
//! This module handles the setup and configuration of OpenTelemetry tracing
//! for the Arda Credit API. It exports traces to the existing OTEL collector
//! running in Kubernetes, which then forwards to SigNoz Cloud.

use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use tracing::{error, info};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

/// Initialize complete tracing setup with OpenTelemetry integration
///
/// This function sets up both the tracing subscriber and OpenTelemetry integration.
/// Traces are sent to the local OTEL collector which handles SigNoz forwarding.
///
/// Environment variables:
/// - `ENABLE_OPENTELEMETRY`: Enable/disable tracing (default: false)
/// - `OTEL_SERVICE_NAME`: Service name for traces (default: "arda-credit-api")
/// - `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP endpoint (default: "http://localhost:4317")
pub fn init_tracing() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

        return Ok(());
    }

    info!("🔧 Initializing OpenTelemetry tracing");

    // Get configuration from environment
    let service_name =
        std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "arda-credit-api".to_string());

    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    info!("📊 OpenTelemetry configuration:");
    info!("   Service Name: {}", service_name);
    info!("   Endpoint: {}", endpoint);

    // Create service resource
    let resource = opentelemetry::sdk::Resource::new(vec![
        KeyValue::new("service.name", service_name),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ]);

    // Initialize tracer pipeline - simple OTLP export to local collector
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint),
        )
        .with_trace_config(
            opentelemetry::sdk::trace::config()
                .with_resource(resource)
                .with_sampler(opentelemetry::sdk::trace::Sampler::AlwaysOn),
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    // Create the OpenTelemetry tracing layer
    let telemetry_layer = OpenTelemetryLayer::new(tracer);

    // Initialize the tracing subscriber with both fmt and OpenTelemetry layers
    Registry::default()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry_layer)
        .init();

    info!("✅ OpenTelemetry tracing initialized successfully");
    Ok(())
}

/// Initialize the OpenTelemetry tracer (for backward compatibility)
///
/// This function is kept for backward compatibility but delegates to init_tracing.
/// Use init_tracing() directly for new code.
pub fn init_tracer() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_tracing()
}

/// Shutdown the OpenTelemetry tracer provider
///
/// This should be called during application shutdown to ensure all spans
/// are properly flushed and exported.
pub async fn shutdown_telemetry() {
    info!("🔄 Shutting down OpenTelemetry");

    // Check if tracing was enabled
    let enable_tracing = std::env::var("ENABLE_OPENTELEMETRY")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    if enable_tracing {
        // Simple shutdown - just signal the global provider
        global::shutdown_tracer_provider();
        info!("✅ OpenTelemetry tracer provider shutdown complete");
    } else {
        info!("✅ OpenTelemetry shutdown complete (was disabled)");
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