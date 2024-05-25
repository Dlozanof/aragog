use color_eyre::Report;

pub trait ShopParser {
    fn process(&self, client: &reqwest::blocking::Client, url: &str, limit: i32) -> Result<(), Report>;
}
