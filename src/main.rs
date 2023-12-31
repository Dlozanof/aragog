use color_eyre::Report;
use tracing::info;
use tracing_subscriber::filter::EnvFilter;
use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Report> {
    setup()?;

    let client = Client::new();
    aragog::parser::
    process_dracotienda(&client, "https://dracotienda.com/1715-juegos-de-tablero").await?;

    Ok(())
}

fn setup() -> Result<(), Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }
    color_eyre::install()?;

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info")
    }
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    Ok(())
}
