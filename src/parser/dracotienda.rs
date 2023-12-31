use crate::types::Offer;
use color_eyre::Report;
use tracing::{info, warn, error};
use reqwest::Client;
use scraper::{Html, Selector};

async fn process_page(body: &String) -> Option<String> {
    let fragment = Html::parse_document(&body);

    let entries = Selector::parse("div.laberProduct-container").unwrap();

    // Process offers in current page
    for entry in fragment.select(&entries) {

        // Get name
        let name_selector = Selector::parse("h2.productName").unwrap();
        let name_tokens: Vec<_> = entry.select(&name_selector).collect();
        if name_tokens.is_empty() {
            continue;
        }
        let name: Option<String> = match name_tokens.first() {
            Some(value) => Some(value.text().collect::<Vec<_>>().get(0).unwrap().to_string()),
            None => None,
        };
    
        if name == None {
            error!("Unable to get name for {:?}", name_tokens);
            continue;
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
            warn!("Bad link for {}", name.unwrap());
            continue;
        }

        // Get current price
        let current_price_selector = Selector::parse("span.price").unwrap();
        let current_price = entry.select(&current_price_selector).next().map(|price| price.text().collect::<String>());
        if current_price == None {
            warn!("Bad current_price for {} [{:?}]", name.unwrap(), current_price);
            continue;
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

        // Send it to backend
        let response = reqwest::Client::new()
            .post("http://localhost:9987/new_offer")
            .header("Content-Type", "application/json")
            .form(&current_offer)
            .send()
            .await;
        if response.unwrap().status() != 200 {
            warn!("Failed to register {:?}", current_offer);
        }
    }

    // Search "next" link and return it
    let next_url_selector = Selector::parse("a.next").unwrap();
    let next_url = fragment.select(&next_url_selector).next().map(|url| url.value().attr("href")).unwrap();
    match next_url {
        Some(url) => Some(url.to_owned()),
        None => None,
    }
}

pub async fn process_dracotienda(client: &Client, url: &str) -> Result<(), Report> {
    
    let mut url_to_process = url.to_owned();
    loop {
        info!("Processing URL [{}]", url);
        let res = client.get(url_to_process).send().await?.error_for_status()?;
        assert!(res.status().is_success());

        let body = res.text().await?;
        
        match process_page(&body).await {
            Some(next_url) => url_to_process = next_url,
            None => break,
        };
    }

    Ok(())
}

fn parse_price(input: &String) -> f32 {
    //input[0..input.len() - 5].replace(",",".").parse::<f32>().unwrap()
    let val = input.split(" ").next().unwrap();
    let val_clean = val.replace(|c: char| !c.is_ascii(), "").replace(",",".");
    let val_float = val_clean.parse::<f32>().unwrap();

    val_float
}
