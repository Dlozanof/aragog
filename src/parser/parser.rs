use crate::types::{PageEntry, Page, Offer};
use color_eyre::Report;
use tracing::{info, warn};
use reqwest::Client;
use scraper::{Html, Selector};

fn process_page(body: &String) -> String {
    let fragment = Html::parse_document(&body);

    let entries = Selector::parse("div.laberProduct-container").unwrap();

    let page_entry = Page {
        next_url:  String::from("TBD"),
        entries: Vec::new(),
    };

    // Process offers in current page
    for entry in fragment.select(&entries) {

        // Get name
        let name_selector = Selector::parse("h2.productName").unwrap();
        let name_tokens: Vec<_> = entry.select(&name_selector).collect();
        let name: Option<String> = match name_tokens.first() {
            Some(value) => Some(value.text().collect::<Vec<_>>().get(0).unwrap().to_string()),
            None => None,
        };
    
        if name == None {
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
            warn!("Bad current_price for {}", name.unwrap());
            continue;
        }
        
        // Get offer price
        let regular_price_selector = Selector::parse("span.regular-price").unwrap();
        let regular_price = entry.select(&regular_price_selector).next().map(|price| price.text().collect::<String>());
        if regular_price == None {
            warn!("Bad regular_price for {}", name.unwrap());
            continue;
        }

        info!("{} | {} | {} -> {}", 
            parse_price(&regular_price.unwrap()),
            parse_price(&current_price.unwrap()),
            name.unwrap(),
            link.unwrap());
    }

    info!("{:?}", page_entry);
    
    // Search "next" link and return it
    let next_url_selector = Selector::parse("a.next").unwrap();
    let next_url = fragment.select(&next_url_selector).next().map(|url| url.value().attr("href")).unwrap().unwrap();
    info!("{:?}", next_url);

    next_url.to_owned()
}

pub async fn fetch_dracotienda(client: &Client, url: &str) -> Result<(), Report> {
    
    let mut url_to_process = url.to_owned();
    loop {
        let res = client.get(url_to_process).send().await?.error_for_status()?;
        assert!(res.status().is_success());

        let body = res.text().await?;
        
        url_to_process = process_page(&body);
    }

    


    Ok(())
}

fn parse_price(input: &String) -> f32 {
    input[0..input.len() - 5].replace(",",".").parse::<f32>().unwrap()
}
