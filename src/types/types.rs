// Given an URL, a page is formed with information about next URL and a list of page_entries to
// scrap

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
