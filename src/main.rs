use color_eyre::Report;
use tracing::info;
use aragog::configuration::get_configuration;
use aragog::parser::{Configuration, DracotiendaParser, ShopParser};
use aragog::telemetry::init_telemetry;
use argh::FromArgs;

#[derive(FromArgs)]
/// Reach new heights.
struct AppParams {
    /// limits the number of offers to be analyzed (aprox)
    #[argh(option, default = "70")]
    limit: i32,
}

#[tokio::main]
async fn main() -> Result<(), Report> {
    //setup()?;

    // Read configuration
    let configuration = get_configuration().expect("Failed to read configuration file");
    // Setup telemetry
    init_telemetry(&configuration.telemetry.endpoint, &configuration.telemetry.service_name);

    // Provide it to different parsers
    let cfg = Configuration {
        server_address: String::from(configuration.backend.url),
        post_endpoint: String::from(configuration.backend.ep),
    };

    // Argument parsing
    let up: AppParams = argh::from_env();

    let _ = std::thread::spawn(move || {
        info!("Thread {} started", 0);

        // Load up parsers
        let mut parser_vector: Vec<Box<dyn ShopParser>> = Vec::new();
        parser_vector.push(
            Box::new(DracotiendaParser { cfg }),
        );

        for parser in parser_vector {
            info!("Processing...");
            let _ = parser.process(&reqwest::blocking::Client::new(), "https://dracotienda.com/1715-juegos-de-tablero", up.limit);
        }
    }).join();



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
    //tracing_subscriber::fmt::fmt()
    //    .with_env_filter(EnvFilter::from_default_env())
    //    .init();

    Ok(())
}
