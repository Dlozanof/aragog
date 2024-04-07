#[derive(serde::Deserialize)]
pub struct Settings {
    pub backend: BackendSettings,
}

#[derive(serde::Deserialize)]
pub struct BackendSettings {
    pub url: String,
}

impl BackendSettings {
    pub fn new_offer_string(&self) -> String {
        format!(
            "{}/new_offer",
            self.url
        )
    }
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let mut settings = config::Config::default();

    settings.merge(config::File::with_name("configuration"))?;

    settings.try_into()
}