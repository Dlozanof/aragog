use color_eyre::Report;
use tracing_subscriber::filter::EnvFilter;
use aragog::configuration::get_configuration;
use aragog::parser::{Configuration, DracotiendaParser, ShopParser};

fn main() -> Result<(), Report> {
    setup()?;

    // Read configuration
    let configuration = get_configuration().expect("Failed to read configuration file");

    // Provide it to different parsers
    let cfg = Configuration {
        server_address: String::from(configuration.backend.url),
        post_endpoint: String::from(configuration.backend.ep),
    };

    // Load up parsers
    let mut parser_vector: Vec<Box<dyn ShopParser>> = Vec::new();
    parser_vector.push(
        Box::new(DracotiendaParser { cfg }),
    );
    //process_dracotienda(&client, "https://dracotienda.com/1715-juegos-de-tablero").await?;

    for parser in parser_vector {
        let _ = parser.process(&reqwest::blocking::Client::new(), "https://dracotienda.com/1715-juegos-de-tablero");
    }

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
