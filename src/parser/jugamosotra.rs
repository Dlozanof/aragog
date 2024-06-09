use crate::types::Offer;
use chrono::DateTime;
use chrono::Utc;
use color_eyre::Report;
use config::Value;
use tracing::{info, warn, error};
use scraper::{ElementRef, Html, Selector};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use crate::parser::ShopParser;
use crate::parser::Configuration;
use tracing::instrument;
use crate::telemetry::{PropagationContext, SpannedMessage};
use regex::Regex;

#[derive(Debug)]
pub struct JugamosotraParser {
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
    let result = str::replace(&result, "(inglés)", "");

    // At this point just remove any parentheses left. Thanks CHATGPT
    let re = Regex::new(r"\([^)]*\)").unwrap();
    let result = re.replace_all(&result, "").to_string();


    Some(result)
}


/* This shop sometimes limits shortens the game and adds `...` to it, so we need
 * to enter into the offer URL and check it by hand. This function just returns
 * the name, the rest of data can be parsed from the main page.
 */
fn process_single_game(url: &str) -> Option<String> {

    // Create a delay
    std::thread::sleep(std::time::Duration::from_secs(5));

    // Request data
    let client = reqwest::blocking::Client::new();
    
    // TODO: Put this in a loop, sometimes we get err 500
    let response = match client
        .get(url)
        .timeout(std::time::Duration::from_secs(600))
        .send() {
        Ok(val) => {
            if val.status() != 200 {
                error!("Failed to get data for single game {}", val.status());
            }
            val
        },
        Err(e) => {
            error!("{}", e.to_string());
            return None;
        }
    };

    // Process the document
    let response_document = match response.text() {
        Ok(text) => Html::parse_document(&text),
        Err(_) => return None
    };
    
    // Extract the name
    let name_selector = Selector::parse("h1.h1[itemprop='name']").unwrap();
    let name;
    if let Some(element) = response_document.select(&name_selector).next() {
        name = element.text().collect::<Vec<_>>().concat();
    } else {
        error!("Product name not found");
        return None;
    }

    info!("Processed name from {} into {}", url, name);

    Some(String::from(name))
}


impl JugamosotraParser {
    
    pub fn new(cfg: Configuration) -> JugamosotraParser {
        JugamosotraParser {cfg}
    }

    #[instrument(level = "info", name = "Processing entry", skip(self, entry), fields(error_detail="OK", shop="JugamosOtra"))]
    fn process_entry(&self, entry: ElementRef, url: &str, batch_name: &str) {

        // Get availability
        let mut availability = "Disponible";
        let availability_selector = Selector::parse("li").unwrap();
        for element in entry.select(&availability_selector) {
            for attr in element.value().attrs() {
                //if String::from("product-flag agotado") == String::from(attr.1) {
                if "product-flag agotado" == attr.1 {
                    availability = "Agotado";
                }
            }
        }

        // ChatGPT boyyyyy
        // Define the CSS selectors
        let name_selector = Selector::parse(".product-title a").unwrap();
        let offer_price_selector = Selector::parse(".product-price-and-shipping .price").unwrap();
        let current_price_selector = Selector::parse(".product-price-and-shipping .regular-price").unwrap();
        let url_selector = Selector::parse(".product-title a").unwrap();

        // Extract and print the offer price
        let offer_price;
        if let Some(element) = entry.select(&offer_price_selector).next() {
            let tmp = element.text().collect::<Vec<_>>().concat();
            offer_price = parse_price(&tmp);
            info!("Offer price: {}", offer_price);
        } else {
            error!("Offer price not found");
            return;
        }

        // Extract and print the current price
        let normal_price;
        if let Some(element) = entry.select(&current_price_selector).next() {
            let tmp = element.text().collect::<Vec<_>>().concat();
            normal_price = parse_price(&tmp);
            info!("Current price: {}", normal_price);
        } else {
            // Game has no offer, so just repeat normal price
            normal_price = offer_price;
        }

        // Extract and print the URL of the offer
        let link;
        if let Some(element) = entry.select(&url_selector).next() {
            if let Some(url) = element.value().attr("href") {
                link = url.to_string();
                info!("Offer URL: {}", link);
            }
            else {
                error!("Unable to parse offer URL");
                return;
            }
        } else {
            error!("Offer URL not found");
            return;
        }

        // Extract and print the product name
        let mut name;
        if let Some(element) = entry.select(&name_selector).next() {
            name = element.text().collect::<Vec<_>>().concat();
        } else {
            error!("Product name not found");
            return;
        }
        if name.contains("...") {
            name = match process_single_game(link.as_str()) {
                Some(name) => name,
                None => {
                    error!("Unable to parse game name from {}", url);
                    return;
                }
            };
        }
        
        let link = match entry.select(&url_selector).next() {
            Some(url) => {
                match url.value().attr("href") {
                    Some(url) => url,
                    None => {
                        error!("No http url in the element");
                        return;
                    }
                }
            }
            None => {
                error!("No url in name");
                return;
            }
        };

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
            shop_name: "JugamosOtra".to_string(),
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

        let entries = Selector::parse("div.thumbnail-container").unwrap();

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


fn parse_price(input: &String) -> f64 {
    let val = input.split(" ").next().unwrap();
    let val_clean = val.replace(|c: char| !c.is_ascii(), "").replace(",",".");
    let val_float = val_clean.parse::<f64>().unwrap();

    val_float
}


impl ShopParser for JugamosotraParser {

    fn process(&self, client: &reqwest::blocking::Client, url: &str, limit: i32) -> Result<(), Report> {
    
        // Epoch information
        let now: DateTime<Utc> = Utc::now();
        let formatted_now = now.format("%Y-%m-%d_%H").to_string();

        let mut url_to_process = url.to_owned();
        let limit = limit / 80 + 1;

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