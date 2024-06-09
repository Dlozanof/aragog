use color_eyre::Report;
use tracing::info;
use aragog::configuration::get_configuration;
use aragog::parser::{Configuration, DracotiendaParser, ShopParser, JugamosotraParser, DungeonMarvelsParser};
use aragog::telemetry::init_telemetry;
use argh::FromArgs;

#[derive(FromArgs)]
/// Reach new heights.
struct AppParams {
    /// limits the number of offers to be analyzed (aprox)
    #[argh(option, default = "70")]
    limit: i32,

    /// which shop to analyze, can be `all` for all of them to run
    #[argh(option, default = "String::from(\"all\")")]
    shop: String
}


// Helper macro, just for the sake of learning
macro_rules! shop {
    // `()` indicates that the macro takes no argument.
    ($thread_vector:ident, $parser:ty, $url:literal, $limit:expr) => {

        $thread_vector.push(std::thread::spawn(move || {
            // TODO: Super sub-optimal, reading the config file several times
            // Read configuration
            let configuration = get_configuration().expect("Failed to read configuration file");
            let cfg = Configuration {
                server_address: String::from(configuration.backend.url.clone()),
                post_endpoint: String::from(configuration.backend.ep.clone()),
            };
            let parser = <$parser>::new(cfg);
            let _ = parser.process(&reqwest::blocking::Client::new(), $url, $limit);
        }));
    };
}

#[tokio::main]
async fn main() -> Result<(), Report> {
    //setup()?;

    // Setup telemetry
    let configuration = get_configuration().expect("Failed to read configuration file");
    init_telemetry(&configuration.telemetry.endpoint, &configuration.telemetry.service_name);

    // Argument parsing
    let up: AppParams = argh::from_env();
    
    // Accumulate children
    let mut children = vec![];

    // Note the Domain Specific Language - ish macros
    match up.shop.as_str() {
        "all" => {
            shop!(children, DracotiendaParser, "https://dracotienda.com/1715-juegos-de-tablero", up.limit);
            shop!(children, JugamosotraParser, "https://jugamosotra.com/es/24-juegos?order=product.sales.desc", up.limit);
            shop!(children, DungeonMarvelsParser, "https://dungeonmarvels.com/10-juegos-de-tablero", up.limit);
        }
        "dracotienda" => {
            shop!(children, DracotiendaParser, "https://dracotienda.com/1715-juegos-de-tablero", up.limit);
        }
        "jugamosotra" => {
            shop!(children, JugamosotraParser, "https://jugamosotra.com/es/24-juegos?order=product.sales.desc", up.limit);
        }
        "dungeonmarvels" => {
            shop!(children, DungeonMarvelsParser, "https://dungeonmarvels.com/10-juegos-de-tablero", up.limit);
        }
        &_ => {
            tracing::error!("Bad option {}", up.shop);
            return Ok(());
        }
    }

    // Wait fot the analysis to finish
    for child in children {
        let _ = child.join();
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
    //tracing_subscriber::fmt::fmt()
    //    .with_env_filter(EnvFilter::from_default_env())
    //    .init();

    Ok(())
}
