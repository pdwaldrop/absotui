use crate::api::library_items::get_pod_ep::Root;
use crate::utils::convert_seconds::convert_seconds;

/// collect title podact episode
pub async fn collect_titles_pod_ep(item: &Root) -> Vec<String> {
    let mut titles_pod_ep = Vec::new();

    if let Some(media) = &item.media
        && let Some(episodes) = &media.episodes {
            for episode in episodes {
                if let Some(title) = &episode.title {
                    titles_pod_ep.push(title.clone());
                } else {
                    titles_pod_ep.push("N/A".to_string());
                }
            }
        }

    titles_pod_ep
}

/// collect ID of podcast episode
pub async fn collect_ids_pod_ep(item: &Root) -> Vec<String> {
    let mut ids_pod_ep = Vec::new();

    if let Some(media) = &item.media
        && let Some(episodes) = &media.episodes {
            for episode in episodes {
                if let Some(id) = &episode.id {
                    ids_pod_ep.push(id.clone());
                } else {
                    ids_pod_ep.push("N/A".to_string());
                }

            }
        }

    ids_pod_ep
}


/// collect subtiles
pub async fn collect_subtitles_pod_ep(item: &Root) -> Vec<String> {
    let mut subtitles_pod_ep = Vec::new();

    if let Some(media) = &item.media
        && let Some(episodes) = &media.episodes {
            for episode in episodes {
                if let Some(sub) = &episode.subtitle {
                    subtitles_pod_ep.push(sub.clone());
                } else {
                    subtitles_pod_ep.push("N/A".to_string());
                }

            }
        }

    subtitles_pod_ep
}

/// collect seasons
pub async fn collect_seasons_pod_ep(item: &Root) -> Vec<String> {
    let mut seasons_pod_ep = Vec::new();

    if let Some(media) = &item.media
        && let Some(episodes) = &media.episodes {
            for episode in episodes {
                if let Some(season) = &episode.season {
                    seasons_pod_ep.push(season.clone());
                } else {
                    seasons_pod_ep.push("N/A".to_string());
                }

            }
        }

    seasons_pod_ep
}

/// collect episodes
pub async fn collect_episodes_pod_ep(item: &Root) -> Vec<String> {
    let mut episodes_pod_ep = Vec::new();

    if let Some(media) = &item.media
        && let Some(episodes) = &media.episodes {
            for episode in episodes {
                if let Some(episode) = &episode.episode {
                    episodes_pod_ep.push(episode.clone());
                }
                else {
                    episodes_pod_ep.push("N/A".to_string());
                }
            }
        }

    episodes_pod_ep
}

/// collect authors
pub async fn collect_authors_pod_ep(item: &Root) -> Vec<String> {
    let mut authors_pod_ep = Vec::new();

    if let Some(media) = &item.media
        && let Some(metadata) = &media.metadata {
            if let Some(author) = &metadata.author {
                authors_pod_ep.push(author.clone());
            } else {
                authors_pod_ep.push("N/A".to_string());
            }

        }

    authors_pod_ep
}

/// collect each episode's own description - `Episode.description`, not the podcast's
/// own `Media.metadata.description` this used to read (the wrong, show-level field,
/// and a single value rather than one per episode). Mirrors `collect_subtitles_pod_ep`'s
/// loop shape exactly so this stays the same length as every other per-episode field.
pub async fn collect_descs_pod_ep(item: &Root) -> Vec<String> {
    let mut descs_pod_ep = Vec::new();

    if let Some(media) = &item.media
        && let Some(episodes) = &media.episodes {
            for episode in episodes {
                if let Some(desc) = &episode.description {
                    descs_pod_ep.push(desc.clone());
                } else {
                    descs_pod_ep.push("N/A".to_string());
                }
            }
        }

    descs_pod_ep
}

/// collect title of podcast (no of podcast episode)
pub async fn collect_titles_pod(item: &Root) -> Vec<String> {
    let mut titles_pod = Vec::new();

    if let Some(media) = &item.media
        && let Some(metadata) = &media.metadata {
            if let Some(title) = &metadata.title {
                titles_pod.push(title.clone());
            } else {
                titles_pod.push("N/A".to_string());
            }

        }

    titles_pod
}

pub async fn collect_durations_pod_ep(item: &Root) -> Vec<String> {
    let mut durations = Vec::new();

    if let Some(media) = &item.media
        && let Some(episodes) = &media.episodes {
            for episode in episodes {
                if let Some(audio_file) = &episode.audio_file {
                    if let Some(duration) = audio_file.duration {
                        durations.push(duration);
                    } else {
                        durations.push(0.0);
                    }

                }
            }
        }

    
    convert_seconds(durations)
}

