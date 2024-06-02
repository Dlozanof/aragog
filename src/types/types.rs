// Given an URL, a page is formed with information about next URL and a list of page_entries to
// scrap

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offer {
    pub url: String,
    pub name: String,
    pub normal_price: f64,
    pub offer_price: f64,
    pub availability: String,
    pub shop_name: String,
}

#[derive(Debug)]
pub struct PageEntry {
    pub url: String,
    pub name: String
}

#[derive(Debug)]
pub struct Page {
    pub next_url: String,
    pub entries: Vec<PageEntry>
}
