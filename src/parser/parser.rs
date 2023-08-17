use crate::types::{PageEntry, Page};
use color_eyre::Report;
use tracing::info;
use reqwest::Client;
use scraper::{Html, Selector};

pub async fn fetch_page_data(client: &Client, url: &str) -> Result<(), Report> {
    let res = client.get(url).send().await?.error_for_status()?;
    assert!(res.status().is_success());

    let body = res.text().await?;
    let fragment = Html::parse_document(&body);

    let entries = Selector::parse("h2.productName").unwrap();

    let mut page_entry = Page {
        next_url:  String::from("TBD"),
        entries: Vec::new(),
    };

    for entry in fragment.select(&entries) {
        let name = entry.text().collect::<Vec<_>>();

        let link_selector = Selector::parse("a").unwrap();
        for link in entry.select(&link_selector) {
            if let Some(href) = link.value().attr("href") {
                page_entry.entries.push(PageEntry {
                    name: String::from(name[0]),
                    url: String::from(href),
                });
                info!("Entry detected! Name: {} | URL: {}", name[0], href);
            }
        }
    }

    info!("{:?}", page_entry);

    Ok(())
}

