use crate::types::Offer;
use chrono::DateTime;
use chrono::Utc;
use color_eyre::Report;
use tracing::{info, warn, error};
use scraper::{ElementRef, Html, Selector};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use crate::parser::ShopParser;
use crate::parser::Configuration;
use tracing::instrument;
use crate::telemetry::{PropagationContext, SpannedMessage};
use regex::Regex;

#[derive(Debug)]
pub struct DungeonMarvelsParser {
    pub cfg: Configuration,
}

// TODO: Implement a blacklist module in which you provide a list of `r""`
// and any of them that matches makes a return None happen
fn process_name(name: &str) -> Option<String> {

    // Any "Preventa" game is automatically out
    let re = Regex::new(r"[pP]reventa").unwrap();
    if re.captures(name).is_some() {
        info!("Preventa game skipped");
        return None;
    }

    // Any "Promo" game is automatically out
    let re = Regex::new(r"[pP]romo").unwrap();
    if re.captures(name).is_some() {
        info!("Promo game skipped");
        return None;
    }
    
    // Any "Expansion" combo shit: you guessed it, jail
    let re = Regex::new(r"[eE]xpansi").unwrap();
    if re.captures(name).is_some() {
        info!("Expansion combo skipped");
        return None;
    }

    // Language in parentheses is just removed
    let result = str::replace(name, "(castellano)", "");
    let result = str::replace(&result, "(Castellano)", "");
    let result = str::replace(&result, "(SEMINUEVO)", "");
    let result = str::replace(&result, "(inglés)", "");
    let result = str::replace(&result, "(Inglés)", "");

    // At this point just remove any parentheses left. Thanks CHATGPT
    let re = Regex::new(r"\([^)]*\)").unwrap();
    let result = re.replace_all(&result, "").to_string();


    Some(result)
}


impl DungeonMarvelsParser {
    
    pub fn new(cfg: Configuration) -> DungeonMarvelsParser {
        DungeonMarvelsParser {cfg}
    }

    #[instrument(level = "info", name = "Processing entry", skip(self, entry), fields(error_detail="OK", shop="DungeonMarvels"))]
    fn process_entry(&self, entry: ElementRef, url: &str, batch_name: &str) {

        // Define the selectors
        let name_selector = Selector::parse("h2.product-title a").unwrap();
        let normal_price_selector = Selector::parse(".regular-price").unwrap();
        let discounted_price_selector = Selector::parse(".price").unwrap();
        let availability_selector = Selector::parse("div.stock-product span.stock-tag").unwrap();
        let url_selector = Selector::parse("div.thumbnail-container a.thumbnail").unwrap();


        // Extract the name of the game
        let name;
        if let Some(element) = entry.select(&name_selector).next() {
            name = element.text().collect::<Vec<_>>().concat();
            info!("Game Name: {}", name);
        } else {
            info!("Game Name not found");
            return;
        }

        // TODO: Decide wether to handle names with `...`
        if name.contains("...") {
            tracing::Span::current().record("error_detail", "dots_in_name");
            error!("Dots in name!");
            return;
        }

        // Extract the discounted price
        let offer_price;
        if let Some(element) = entry.select(&discounted_price_selector).next() {
            let tmp = element.text().collect::<Vec<_>>().concat();
            offer_price = parse_price(&tmp);
            info!("Offer price: {}", offer_price);
        } else {
            error!("Offer Price not found");
            return;
        }

        // Extract the current price
        let mut normal_price = offer_price;
        if let Some(element) = entry.select(&normal_price_selector).next() {
            let tmp = element.text().collect::<Vec<_>>().concat();
            normal_price = parse_price(&tmp);
            info!("Current price: {}", normal_price);
        } else {
            info!("Current Price not found");
        }

        // Extract the availability status
        let mut availability = String::from("Available");
        if let Some(element) = entry.select(&availability_selector).next() {
            availability = element.text().collect::<Vec<_>>().concat().trim().to_string();
            info!("Availability: {}", availability);
        } else {
            info!("Availability not found");
        }

        // Extract the URL
        let link;
        if let Some(element) = entry.select(&url_selector).next() {
            link = element.value().attr("href").unwrap_or("N/A");
            info!("URL: {}", url);
        } else {
            error!("URL not found");
            return;
        }

        // Name cleaning
        info!("Processing {}", name);

        // Process name, remove weird offers
        let name = match process_name(name.as_str()) {
            Some(name) => name,
            None => {
                return;
            }
        };
        info!("Game processed to {}", name);

        // Create the object offer
        let current_offer = Offer {
            name,
            url: link.to_string(),
            offer_price,
            normal_price,
            availability: availability.to_owned(),
            shop_name: "DungeonMarvels".to_string(),
        };
        info!("{:?}", current_offer);

        let propagation_context = PropagationContext::inject(&tracing::Span::current().context());
        let spanned_message = SpannedMessage::new(propagation_context, current_offer.clone());

        let post_url = format!("{}/{}", self.cfg.server_address, self.cfg.post_endpoint);

        let response = reqwest::blocking::Client::new()
            .post(post_url)
            .header("Content-Type", "application/json")
            .json(&spanned_message)
            .timeout(std::time::Duration::from_secs(600))
            .send();
        match response {
            Ok(val) => {
                if val.status() == 515 {
                    warn!("Unable to match {:?}", current_offer);
                }
                else if val.status() != 200 {
                    error!("{} Failed to register {:?}", val.status(), current_offer);

                    // TODO: Fix this issue, but for now monitor it
                    if val.status() == 408 {
                        tracing::Span::current().record("error_detail", "HttpTimeout");
                    }
                }
                else {
                    info!("Registered!");
                }
            },
            Err(e) => {
                error!("{}", e.to_string());
            }
        }
    }

    fn process_page(&self, body: &String, url: &str, batch_name: &str) -> Option<String> {
        let fragment = Html::parse_document(&body);

        let entries = Selector::parse("div.product-container").unwrap();

        // Process offers in current page
        for entry in fragment.select(&entries) {
            self.process_entry(entry, url, batch_name);
        }

        // Search "next" link and return it
        let next_url_selector = Selector::parse("a.next").unwrap();
        let next_url = fragment.select(&next_url_selector).next().map(|url| url.value().attr("href")).unwrap();
        match next_url {
            Some(url) => Some(url.to_owned()),
            None => None,
        }
    }
}


fn parse_price(input: &str) -> f64 {
    let val = input.split(" ").next().unwrap();
    let val_clean = val.replace(|c: char| !c.is_ascii(), "").replace(",",".");
    let val_float = val_clean.parse::<f64>().unwrap();

    val_float
}


impl ShopParser for DungeonMarvelsParser {

    fn process(&self, client: &reqwest::blocking::Client, url: &str, limit: i32) -> Result<(), Report> {
    
        // Epoch information
        let now: DateTime<Utc> = Utc::now();
        let formatted_now = now.format("%Y-%m-%d_%H").to_string();

        let mut url_to_process = url.to_owned();
        let limit = limit / 24 + 1;

        let mut loop_limiter = 3;

        for _ in 0..limit {
            // Check for amount of retries
            if loop_limiter == 0 {
                break;
            }

            // Try to get a response from the web
            let response = match client
                .get(url_to_process.clone())
                .timeout(std::time::Duration::from_secs(600))
                .send() {
                Ok(val) => {
                    if val.status() != 200 {
                        error!("Failed to get data from shop {}", val.status());
                        loop_limiter = loop_limiter - 1;
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        continue;
                    }
                    loop_limiter = 3; // Reset the limiter
                    val
                },
                Err(e) => {
                    error!("{}", e.to_string());
                    loop_limiter = loop_limiter - 1;
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    continue;
                }
            };
    
            let body = response.text()?;
            
            match self.process_page(&body, &url, &formatted_now) {
                Some(next_url) => url_to_process = next_url,
                None => break,
            };
        }
    
        Ok(())
    }
}
