#[derive(serde::Deserialize)]
pub struct Settings {
    pub backend: BackendSettings,
    pub telemetry: TelemetrySettings,
}

#[derive(serde::Deserialize)]
pub struct BackendSettings {
    pub url: String,
    pub ep: String,
}

#[derive(serde::Deserialize)]
pub struct TelemetrySettings {
    pub endpoint: String,
    pub service_name: String,
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let mut settings = config::Config::default();

    settings.merge(config::File::with_name("configuration"))?;

    settings.try_into()
}