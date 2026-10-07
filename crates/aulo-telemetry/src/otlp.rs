//! OTLP/HTTP trace export, built only with the `otlp` feature.

use std::time::Duration;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::{
    BatchSpanProcessor, SdkTracerProvider, Span, SpanData, SpanProcessor,
};
use tracing_subscriber::Layer;
use tracing_subscriber::registry::Registry;

use crate::redact::{REDACTED, is_secret_name, scrub_text};
use crate::{BoxedLayer, Error};

/// Flushes and shuts the exporter down on drop so the last spans are not lost at exit.
#[derive(Debug)]
pub(crate) struct Provider(SdkTracerProvider);

impl Drop for Provider {
    fn drop(&mut self) {
        // Nowhere to report an export failure at shutdown; the process is exiting anyway.
        let _ = self.0.shutdown();
    }
}

pub(crate) fn layer(endpoint: &str) -> Result<(BoxedLayer, Provider), Error> {
    let exporter = SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| Error::Otlp(e.to_string()))?;
    let provider = SdkTracerProvider::builder()
        .with_span_processor(Scrubbing(BatchSpanProcessor::builder(exporter).build()))
        .with_resource(Resource::builder().with_service_name("aulo").build())
        .build();
    let layer: Box<dyn Layer<Registry> + Send + Sync> =
        Box::new(tracing_opentelemetry::layer().with_tracer(provider.tracer("aulo")));
    Ok((layer, Provider(provider)))
}

/// The OTel layer records span fields itself, bypassing the log-line scrubbing, so spans are
/// masked here, just before they leave the process.
#[derive(Debug)]
struct Scrubbing<P>(P);

impl<P: SpanProcessor> SpanProcessor for Scrubbing<P> {
    fn on_start(&self, span: &mut Span, cx: &opentelemetry::Context) {
        self.0.on_start(span, cx);
    }

    fn on_end(&self, mut span: SpanData) {
        scrub_attributes(&mut span.attributes);
        for event in &mut span.events.events {
            scrub_attributes(&mut event.attributes);
            if let std::borrow::Cow::Owned(name) = scrub_text(&event.name) {
                event.name = name.into();
            }
        }
        self.0.on_end(span);
    }

    fn force_flush(&self) -> OTelSdkResult {
        self.0.force_flush()
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.0.shutdown_with_timeout(timeout)
    }

    fn set_resource(&mut self, resource: &Resource) {
        self.0.set_resource(resource);
    }
}

pub(crate) fn scrub_attributes(attributes: &mut [opentelemetry::KeyValue]) {
    for attribute in attributes {
        if is_secret_name(attribute.key.as_str()) {
            attribute.value = REDACTED.into();
        } else if let opentelemetry::Value::String(text) = &attribute.value
            && let std::borrow::Cow::Owned(masked) = scrub_text(text.as_str())
        {
            attribute.value = masked.into();
        }
    }
}
