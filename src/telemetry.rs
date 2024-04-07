use opentelemetry::propagation::Extractor;
use tracing::subscriber::{set_global_default};
use opentelemetry::{global, KeyValue};
use opentelemetry::sdk::propagation::TraceContextPropagator;
use opentelemetry::sdk::{trace, Resource};
use opentelemetry_otlp::WithExportConfig;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::Registry;
use tracing_subscriber::{prelude::*, EnvFilter};

use opentelemetry::{
    propagation::Injector,
};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

pub fn init_telemetry(exporter_endpoint: &str, service_name: &str) {
    // Create a gRPC exporter
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(exporter_endpoint);

    // Define a tracer
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(
            trace::config().with_resource(Resource::new(vec![KeyValue::new(
                opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                service_name.to_string(),
            )])),
        )
        .install_batch(opentelemetry::runtime::Tokio)
        .expect("Error: Failed to initialize the tracer.");

    // Level filter layer to filter traces based on level (trace, debug, info, warn, error).
    let level_filter_layer = EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("INFO"));
    // Layer for adding our configured tracer.
    let tracing_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    // Layer for printing spans to stdout
    let formatting_layer = BunyanFormattingLayer::new(
        service_name.to_string(),
        std::io::stdout,
    );
    global::set_text_map_propagator(TraceContextPropagator::new());

    let subscriber = Registry::default()
        .with(level_filter_layer)
        .with(tracing_layer)
        .with(JsonStorageLayer)
        .with(formatting_layer);

    // Not sure if this is needed anymore. But I think yes.
    set_global_default(subscriber).expect("Failed to set subscriber");
}


// Let's go crazy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropagationContext{
    //#[serde(with = "string")]
    ctx: HashMap<String, String>
}

impl PropagationContext {

    pub fn inject(context: &opentelemetry::Context) -> Self {
        global::get_text_map_propagator(|propagator| {
            let mut propagation_context = PropagationContext {ctx: HashMap::new()};
            propagator.inject_context(context, &mut propagation_context);
            propagation_context
        })
    }
    pub fn extract(&self) -> opentelemetry::Context {
        global::get_text_map_propagator(|propagator| propagator.extract(self))
    }
}
  
impl Injector for PropagationContext {
    fn set(&mut self, key: &str, value: String) {
        self.ctx.insert(key.to_owned(), value);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpannedMessage<T: core::fmt::Debug + Clone> {
    context: PropagationContext,
    body: T,
}

impl<T: core::fmt::Debug + Clone> SpannedMessage<T> {
    pub fn new(context: PropagationContext, body: T) -> Self {
        Self { context, body }
    }

    pub fn unwrap(self) -> T {
        self.body
    }

    pub fn context(&self) -> &PropagationContext {
        &self.context
    }
}

impl Extractor for PropagationContext {
    fn get(&self, key: &str) -> Option<&str> {
        let key = key.to_owned();
        self.ctx.get(&key).map(|v| v.as_ref())
    }

    fn keys(&self) -> Vec<&str> {
        self.ctx.keys().map(|k| k.as_ref()).collect()
    }
}