use crate::types::Offer;
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
pub struct DracotiendaParser {
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

impl DracotiendaParser {

    #[instrument(level = "info", name = "Processing entry", skip_all)]
    fn process_entry(&self, entry: ElementRef) {

        // Get name
        let name_selector = Selector::parse("h2.productName").unwrap();
        let name_tokens: Vec<_> = entry.select(&name_selector).collect();
        if name_tokens.is_empty() {
            //error!("Name is empty"); // Not sure why but there are several empty offers every
            //page, probably a shitty workaround
            return;
        }
        let name: Option<String> = match name_tokens.first() {
            Some(value) => Some(value.text().collect::<Vec<_>>().get(0).unwrap().to_string()),
            None => None,
        };

        if name.is_none() {
            error!("Unable to get name for {:?}", name_tokens);
            return;
        }

        let name = name.unwrap();
        info!("Processing {}", name);

        // Process name, remove weird offers
        let name = match process_name(name.as_str()) {
            Some(name) => name,
            None => {
                return;
            }
        };
        info!("Game processed to {}", name);

        // Get url
        let link_selector = Selector::parse("a").unwrap();
        let link: Option<String> = match entry.select(&link_selector).collect::<Vec<_>>().first() {
            Some(link_value) => {
                match link_value.value().attr("href") {
                    Some(url) => Some(String::from(url)),
                    None => None,
                }
            }
            None => None,
        };
        if link == None {
            error!("Bad link for {}", name);
            return;
        }

        // Get current price
        let current_price_selector = Selector::parse("span.price").unwrap();
        let current_price = entry.select(&current_price_selector).next().map(|price| price.text().collect::<String>());
        if current_price == None {
            error!("Bad current_price for {} [{:?}]", name, current_price);
            return;
        }

        // Get offer price. If there is none, then is not a discount but a normal offer.
        let regular_price_selector = Selector::parse("span.regular-price").unwrap();
        let regular_price = match entry.select(&regular_price_selector).next().map(|price| price.text().collect::<String>()) {
            Some(price) => Some(price),
            None => current_price.to_owned(),
        };

        // Get availability
        let availability_selector = Selector::parse("span.product-availability").unwrap();
        let mut availability = match entry.select(&availability_selector).next().map(|t| t.text().collect::<String>()) {
            Some(t) => t,
            None => String::new(),
        };
        availability.retain(|c| c.is_alphanumeric() || c.is_whitespace());
        let availability = availability.trim();

        info!("Availability: {}", availability);

        // Create the object offer
        let current_offer = Offer {
            name,
            url: link.unwrap(),
            offer_price: parse_price(&current_price.unwrap()),
            normal_price: parse_price(&regular_price.unwrap()),
            availability: availability.to_owned()
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

    fn process_page(&self, body: &String) -> Option<String> {
        let fragment = Html::parse_document(&body);

        let entries = Selector::parse("div.laberProduct-container").unwrap();

        // Process offers in current page
        for entry in fragment.select(&entries) {
            self.process_entry(entry);
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
    //input[0..input.len() - 5].replace(",",".").parse::<f32>().unwrap()
    let val = input.split(" ").next().unwrap();
    let val_clean = val.replace(|c: char| !c.is_ascii(), "").replace(",",".");
    let val_float = val_clean.parse::<f64>().unwrap();

    val_float
}



impl ShopParser for DracotiendaParser {

    fn process(&self, client: &reqwest::blocking::Client, url: &str, limit: i32) -> Result<(), Report> {
    
        let mut url_to_process = url.to_owned();
        let limit = limit / 20 + 1;
        for _ in 0..limit {
            let res = client.get(url_to_process).send()?.error_for_status()?;
            assert!(res.status().is_success());
    
            let body = res.text()?;
            
            match self.process_page(&body) {
                Some(next_url) => url_to_process = next_url,
                None => break,
            };
        }
    
        Ok(())
    }
}
