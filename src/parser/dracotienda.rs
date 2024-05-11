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

#[derive(Debug)]
pub struct DracotiendaParser {
    pub cfg: Configuration,
}


impl DracotiendaParser {

    #[instrument(level = "info", name = "Processing entry", skip_all)]
    fn process_entry(&self, entry: ElementRef) {

        // Get name
        let name_selector = Selector::parse("h2.productName").unwrap();
        let name_tokens: Vec<_> = entry.select(&name_selector).collect();
        if name_tokens.is_empty() {
            return;
        }
        let name: Option<String> = match name_tokens.first() {
            Some(value) => Some(value.text().collect::<Vec<_>>().get(0).unwrap().to_string()),
            None => None,
        };

        if name == None {
            error!("Unable to get name for {:?}", name_tokens);
            return;
        }

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
            error!("Bad link for {}", name.unwrap());
            return;
        }

        // Get current price
        let current_price_selector = Selector::parse("span.price").unwrap();
        let current_price = entry.select(&current_price_selector).next().map(|price| price.text().collect::<String>());
        if current_price == None {
            error!("Bad current_price for {} [{:?}]", name.unwrap(), current_price);
            return;
        }

        // Get offer price. If there is none, then is not a discount but a normal offer.
        let regular_price_selector = Selector::parse("span.regular-price").unwrap();
        let regular_price = match entry.select(&regular_price_selector).next().map(|price| price.text().collect::<String>()) {
            Some(price) => Some(price),
            None => current_price.to_owned(),
        };

        // Create the object offer
        let current_offer = Offer {
            name: name.unwrap(),
            url: link.unwrap(),
            offer_price: parse_price(&current_price.unwrap()),
            normal_price: parse_price(&regular_price.unwrap()),
        };
        info!("{:?}", current_offer);

        let propagation_context = PropagationContext::inject(&tracing::Span::current().context());
        let spanned_message = SpannedMessage::new(propagation_context, current_offer.clone());

        let post_url = format!("{}/{}", self.cfg.server_address, self.cfg.post_endpoint);

        let response = reqwest::blocking::Client::new()
            .post(post_url)
            .header("Content-Type", "application/json")
            .json(&spanned_message)
            .send();
        match response {
            Ok(val) => {
                if val.status() != 200 {
                    error!("{} Failed to register {:?}", val.status(), current_offer);
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

    fn process(&self, client: &reqwest::blocking::Client, url: &str) -> Result<(), Report> {
    
        let mut url_to_process = url.to_owned();
        loop {
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
