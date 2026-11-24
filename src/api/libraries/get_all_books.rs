use crate::utils::http_client::api_client;
use serde_json::Value;
use reqwest::header::AUTHORIZATION;
use color_eyre::eyre::{Result, Report};
use serde::Deserialize;
use serde::Serialize;

/// Get all books or podcasts from a library
/// <https://api.audiobookshelf.org/#get-a-library-39-s-items>

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub results: Option<Vec<LibraryItem>>,
    pub total: Option<i64>,
    pub limit: Option<i64>,
    pub page: Option<i64>,
    pub sort_by: Option<String>,
    pub sort_desc: Option<bool>,
    pub filter_by: Option<String>,
    pub media_type: Option<String>,
    pub minified: Option<bool>,
    pub collapseseries: Option<bool>,
    pub include: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: Option<String>,
    pub ino: Option<String>,
    pub library_id: Option<String>,
    pub folder_id: Option<String>,
    pub path: Option<String>,
    pub rel_path: Option<String>,
    pub is_file: Option<bool>,
    pub mtime_ms: Option<i64>,
    pub ctime_ms: Option<i64>,
    pub birthtime_ms: Option<i64>,
    pub added_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub is_missing: Option<bool>,
    pub is_invalid: Option<bool>,
    pub media_type: Option<String>,
    pub media: Option<Media>,
    pub num_files: Option<i64>,
    pub size: Option<i64>,
    pub collapsed_series: Option<CollapsedSeries>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub metadata: Option<Metadata>,
    pub cover_path: Option<String>,
    pub tags: Option<Vec<Value>>,
    pub num_tracks: Option<i64>,
    pub num_audio_files: Option<i64>,
    pub num_chapters: Option<i64>,
    pub duration: Option<f64>,
    pub size: Option<i64>,
    pub ebook_file_format: Option<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub title: Option<String>,
    pub title_ignore_prefix: Option<String>,
    pub subtitle: Option<Value>,
    pub author_name: Option<String>,
    pub author: Option<String>,
    pub narrator_name: Option<String>,
    pub series: Option<Vec<Series>>,
    // What servers actually send: a packed string like "Saga #3" or "A #7, B #7". `series` above is kept in
    // case a server sends the structured form; collect_series_library prefers it.
    pub series_name: Option<String>,
    pub genres: Option<Vec<String>>,
    pub published_year: Option<String>,
    pub published_date: Option<Value>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub isbn: Option<Value>,
    pub asin: Option<String>,
    pub language: Option<Value>,
    pub explicit: Option<bool>,
}

// A book can belong to more than one series - only the first entry is used
// (see collect_series_library), matching how Audiobookshelf's own UI surfaces
// a single "primary" series badge.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub id: Option<String>,
    pub name: Option<String>,
    pub sequence: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollapsedSeries {
    pub id: Option<String>,
    pub name: Option<String>,
    pub name_ignore_prefix: Option<String>,
    pub num_books: Option<i64>,
}

pub async fn get_all_books(token: &str, id_selected_lib: &String, server_address: String) -> Result<Root> {
    let client = api_client();
    let url = format!("{server_address}/api/libraries/{id_selected_lib}/items?limit=0");


    let response = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(Report::new(std::io::Error::other(
                    "Failed to fetch data from the API",
        )));
    }

    let library: Root = response.json().await?;

    Ok(library)
}

