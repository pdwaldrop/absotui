use crate::utils::http_client::api_client;
use serde_json::Value;
use reqwest::header::AUTHORIZATION;
use color_eyre::eyre::{Result, Report};
use serde::Deserialize;
use serde::Serialize;
use log::{debug, warn};
use crate::api::me::get_media_progress::{get_episode_progress, Root as Progress};
use crate::utils::http_client::MAX_CONCURRENT_REQUESTS;
use futures::stream::{self, StreamExt};

/// Get a `PersonalizedView`'s Personalized View  for podcast(allow to have continue linstening)
/// <https://api.audiobookshelf.org/#get-a-library-39-s-personalized-view>

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub id: Option<String>,
    pub label: String,
    pub label_string_key: Option<String>,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub entities: Option<Vec<Entity>>,
    pub total: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: Option<String>,
    pub ino: Option<String>,
    pub old_library_item_id: Option<Value>,
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
    pub recent_episode: Option<RecentEpisode>,
    // Not part of the personalized-view API response - filled in manually after fetching,
    // from the same per-episode progress check already done to filter out finished
    // episodes, so we don't need a second round of API calls just to display it.
    #[serde(skip)]
    pub progress_percent: Option<f32>,
    #[serde(skip)]
    pub progress_current_time: Option<f64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub id: Option<String>,
    pub metadata: Option<Metadata>,
    pub cover_path: Option<String>,
    pub tags: Option<Vec<Value>>,
    pub num_episodes: Option<i64>,
    pub auto_download_episodes: Option<bool>,
    pub auto_download_schedule: Option<String>,
    pub last_episode_check: Option<i64>,
    pub max_episodes_to_keep: Option<i64>,
    pub max_new_episodes_to_download: Option<i64>,
    pub size: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub release_date: Option<String>,
    pub genres: Option<Vec<String>>,
    pub feed_url: Option<String>,
    pub image_url: Option<String>,
    pub itunes_page_url: Option<String>,
    pub itunes_id: Option<Value>,
    pub itunes_artist_id: Option<String>,
    pub explicit: Option<bool>,
    pub language: Option<String>,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub title_ignore_prefix: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentEpisode {
    pub library_item_id: Option<String>,
    pub podcast_id: Option<String>,
    pub id: Option<String>,
    pub old_episode_id: Option<Value>,
    pub index: Option<Value>,
    pub season: Option<String>,
    pub episode: Option<String>,
    pub episode_type: Option<String>,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub enclosure: Option<Enclosure>,
    pub guid: Option<String>,
    pub pub_date: Option<String>,
    pub chapters: Option<Vec<Chapter>>,
    pub audio_file: Option<AudioFile>,
    pub published_at: Option<i64>,
    pub added_at: Option<i64>,
    pub updated_at: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Enclosure {
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub length: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub title: Option<String>,
    pub id: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioFile {
    pub index: Option<i64>,
    pub ino: Option<String>,
    pub added_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub track_num_from_meta: Option<Value>,
    pub disc_num_from_meta: Option<Value>,
    pub track_num_from_filename: Option<Value>,
    pub disc_num_from_filename: Option<Value>,
    pub manually_verified: Option<bool>,
    pub exclude: Option<bool>,
    pub error: Option<Value>,
    pub format: Option<String>,
    pub duration: Option<f64>,
    pub bit_rate: Option<i64>,
    pub language: Option<Value>,
    pub codec: Option<String>,
    pub time_base: Option<String>,
    pub channels: Option<i64>,
    pub channel_layout: Option<String>,
    pub embedded_cover_art: Option<Value>,
    pub mime_type: Option<String>,
}

// Combines the "Continue Listening" and "Newest Episodes" shelves from the personalized
// view into one "new & unfinished" list, filtered to exclude already-finished episodes.
pub async fn get_new_and_unfinished_pod(token: &str, server_address: String, id_selected_lib: &String) -> Result<Vec<Root>> {
    let client = api_client();
    let url = format!("{server_address}/api/libraries/{id_selected_lib}/personalized");

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

    let libraries: Vec<Root> = response.json().await?;

    // Combine "Continue Listening" (in-progress) and "Newest Episodes" (never started)
    // into one "new & unfinished" list, rather than just the server's own "Continue
    // Listening" shelf - which by definition only covers already-started episodes, and
    // (per observed behavior) doesn't reliably drop episodes once finished either.
    let mut entities: Vec<Entity> = libraries
        .into_iter()
        .filter(|lib| lib.label == "Continue Listening" || lib.label == "Newest Episodes")
        .filter_map(|lib| lib.entities)
        .flatten()
        .collect();

    // De-duplicate by episode ID - an episode could plausibly appear in both shelves.
    let mut seen_episode_ids = std::collections::HashSet::new();
    entities.retain(|entity| {
        let episode_id = entity.recent_episode.as_ref().and_then(|ep| ep.id.clone());
        match episode_id {
            Some(id) => seen_episode_ids.insert(id),
            None => true,
        }
    });

    // Exclude already-finished episodes. Unlike a book, a podcast episode's progress
    // record can't be found by the bare episode ID alone - it needs the parent podcast's
    // library item ID too (see `get_episode_progress`), the same two-ID shape playback
    // and progress-sync already use. A 404/Err means the episode was never started,
    // which counts as "unfinished", not excluded. The percent/current_time from this
    // same call is stashed on the entity for display, so we don't need a second round
    // of API calls just to show progress.
    // One progress lookup per entity, issued concurrently rather than one-at-a-time -
    // this loop was ~230ms per episode serially and is a meaningful share of startup
    // (see bug_id 3f729c). `buffered` keeps results in request order so each progress
    // record still lines up with the entity it belongs to; `buffer_unordered` would
    // pair progress against the wrong episode. The filtering/logging below stays
    // sequential and unchanged, so ordering and log output are identical to before.
    // Each future owns everything it needs and is built up front, so the stream holds
    // no borrow of the `&str` token param. Capturing that borrow instead (even
    // indirectly, via a closure that clones it) makes every caller's enclosing future
    // fail `Send` inference with a higher-ranked-lifetime error at the `tokio::spawn`
    // sites in app.rs.
    let mut progress_futures = Vec::with_capacity(entities.len());
    for entity in &entities {
        let episode_id = entity.recent_episode.as_ref().and_then(|ep| ep.id.clone());
        let library_item_id = entity.recent_episode.as_ref().and_then(|ep| ep.library_item_id.clone());
        let episode_title = entity.recent_episode.as_ref().and_then(|ep| ep.title.clone()).unwrap_or_default();
        let server_address = server_address.clone();
        let token = token.to_string();
        progress_futures.push(async move {
            match (&library_item_id, &episode_id) {
                (Some(lib_id), Some(ep_id)) => match get_episode_progress(&token, lib_id, ep_id, server_address).await {
                    Ok(p) => p,
                    Err(e) => {
                        warn!("[get_new_and_unfinished_pod] progress lookup failed for '{episode_title}' ({lib_id}/{ep_id}), treating as unfinished: {e}");
                        None
                    }
                },
                _ => None,
            }
        });
    }
    let progresses: Vec<Option<Progress>> = stream::iter(progress_futures)
        .buffered(MAX_CONCURRENT_REQUESTS)
        .collect()
        .await;

    let mut unfinished_entities = Vec::new();
    for (mut entity, progress) in entities.into_iter().zip(progresses) {
        let episode_id = entity.recent_episode.as_ref().and_then(|ep| ep.id.clone());
        let episode_title = entity.recent_episode.as_ref().and_then(|ep| ep.title.clone()).unwrap_or_default();
        let is_finished = progress.as_ref().is_some_and(|p| p.is_finished);
        debug!("[get_new_and_unfinished_pod] '{episode_title}' ({episode_id:?}) isFinished={is_finished}");
        if !is_finished {
            if let Some(p) = &progress {
                entity.progress_percent = Some((p.progress * 100.0) as f32);
                entity.progress_current_time = Some(p.current_time);
            }
            unfinished_entities.push(entity);
        }
    }
    let entities = unfinished_entities;

    let combined = Root {
        id: None,
        label: "New & Unfinished".to_string(),
        label_string_key: None,
        type_field: None,
        total: Some(entities.len() as i64),
        entities: Some(entities),
    };

    Ok(vec![combined])
}

