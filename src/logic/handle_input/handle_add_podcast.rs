use crate::api::libraries::get_all_libraries::get_all_libraries;
use crate::api::podcasts::search_podcast::PodcastSearchResult;
use crate::api::podcasts::create_podcast::{create_podcast, CreatePodcastParams};
use crate::api::podcasts::get_podcast_feed::get_podcast_feed;
use crate::api::podcasts::download_episodes::download_episodes;
use crate::app::PodcastAddOutcome;

/// The actual work behind confirming a podcast add: resolves the target library's
/// first folder (Audiobookshelf needs an explicit folder + on-disk path per item,
/// same as the official web client's own "New Podcast" dialog computes), creates
/// the subscription, then seeds it with `episode_count` episodes.
///
/// Seeding goes through `get_podcast_feed` + `download_episodes`, not
/// `check_new_episodes`/`checknew` - confirmed live that checknew doesn't work for
/// this: Audiobookshelf's own `Podcast.createFromRequest` stamps a freshly-created
/// podcast's `lastEpisodeCheck` to the creation instant (not null/epoch-0 as its
/// name might suggest), and checknew only considers episodes published *after*
/// that timestamp - so a checknew call made immediately after creating one always
/// finds zero episodes, since every episode in any real feed necessarily predates
/// "right now". `download_episodes` takes raw feed-episode objects directly with
/// no date filtering at all, which is what the official web client's own episode
/// list uses to queue a specific episode's download regardless of age.
pub async fn create_and_seed_podcast(chosen: PodcastSearchResult, episode_count: u32, library_id: String, token: String, server_address: String) -> PodcastAddOutcome {
    let libraries = match get_all_libraries(&token, server_address.clone()).await {
        Ok(root) => root.libraries,
        Err(e) => return PodcastAddOutcome::Failed(format!("Couldn't look up your libraries: {e}")),
    };

    let Some(library) = libraries.iter().find(|library| library.id == library_id) else {
        return PodcastAddOutcome::Failed("Couldn't find the current library".to_string());
    };

    let Some(folder) = library.folders.first() else {
        return PodcastAddOutcome::Failed("This library has no folder to add a podcast into".to_string());
    };

    let Some(feed_url) = chosen.feed_url.clone() else {
        return PodcastAddOutcome::Failed("This result has no RSS feed URL".to_string());
    };

    let title = chosen.title.clone().unwrap_or_else(|| "Podcast".to_string());
    // Strip characters that can't live in a filesystem path, same set the official
    // web client itself strips when building a podcast's on-disk folder name.
    let clean_title: String = title.chars().filter(|c| !r#"\/:*?"<>|"#.contains(*c)).collect();
    let path = format!("{}/{}", folder.full_path, clean_title);

    let params = CreatePodcastParams {
        library_id,
        folder_id: folder.id.clone(),
        path,
        title,
        author: chosen.artist_name.clone(),
        description: chosen.description.clone().or(chosen.description_plain.clone()),
        release_date: chosen.release_date.clone(),
        genres: chosen.genres.clone(),
        feed_url,
        image_url: chosen.cover.clone(),
        itunes_page_url: chosen.page_url.clone(),
        itunes_id: chosen.id,
        itunes_artist_id: chosen.artist_id,
        explicit: chosen.explicit,
    };

    let feed_url_for_seeding = params.feed_url.clone();
    let item_id = match create_podcast(params, &token, server_address.clone()).await {
        Ok(id) => id,
        Err(e) => return PodcastAddOutcome::Failed(e.to_string()),
    };

    if episode_count == 0 {
        return PodcastAddOutcome::Created(0);
    }

    // The subscription itself is already created at this point regardless of
    // whether seeding succeeds - report success with however many actually got
    // queued (possibly 0) rather than a failure, since retrying "add" from here
    // would just hit "podcast already exists" on the server.
    let feed = match get_podcast_feed(&feed_url_for_seeding, &token, server_address.clone()).await {
        Ok(feed) => feed,
        Err(e) => {
            log::warn!("[create_and_seed_podcast] created {item_id} but couldn't re-fetch feed to seed episodes: {e}");
            return PodcastAddOutcome::Created(0);
        }
    };

    // Feeds conventionally list newest-first - `episodes` here is exactly what
    // `download_episodes` expects verbatim, so no need to parse/reshape it further.
    let to_download: Vec<serde_json::Value> = feed.episodes.into_iter().take(episode_count as usize).collect();
    if to_download.is_empty() {
        return PodcastAddOutcome::Created(0);
    }

    match download_episodes(&item_id, &to_download, &token, server_address).await {
        Ok(()) => PodcastAddOutcome::Created(to_download.len()),
        Err(e) => {
            log::warn!("[create_and_seed_podcast] created {item_id} but download_episodes failed: {e}");
            PodcastAddOutcome::Created(0)
        }
    }
}
