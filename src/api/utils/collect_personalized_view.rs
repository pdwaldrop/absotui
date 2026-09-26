use crate::api::libraries::get_library_perso_view::Root;

// Every collector here pushes exactly one entry per entity - even when
// media/metadata is missing, in which case a placeholder goes in instead. These
// arrays are indexed in lockstep with siblings that never skip (ids, progress),
// so skipping an entity here would silently misalign every index after it (see
// CLAUDE.md's parallel-arrays warning) - and the pin-to-top reorder in
// render_home indexes some of them directly, which would panic outright.

/// collect titles
pub async fn collect_titles_cnt_list(continue_listening: &[Root]) -> Vec<String> {
    let mut titles_cnt_list = Vec::new();

    for library in continue_listening {
        if let Some(entities) = &library.entities {
            for entity in entities {
                let title = entity.media.as_ref()
                    .and_then(|media| media.metadata.as_ref())
                    .and_then(|metadata| metadata.title.clone());
                titles_cnt_list.push(title.unwrap_or_else(|| "N/A".to_string()));
            }
        }
    }

    titles_cnt_list
}

/// collect author name - one entry per entity, see above.
pub async fn collect_auth_names_cnt_list(continue_listening: &[Root]) -> Vec<String> {
    let mut auth_names_cnt_list = Vec::new();

    for library in continue_listening {
        if let Some(entities) = &library.entities {
            for entity in entities {
                let author_name = entity.media.as_ref()
                    .and_then(|media| media.metadata.as_ref())
                    .and_then(|metadata| metadata.author_name.clone());
                auth_names_cnt_list.push(author_name.unwrap_or_else(|| "N/A".to_string()));
            }
        }
    }

    auth_names_cnt_list
}

/// collect published year - one entry per entity, see above.
pub async fn collect_pub_year_cnt_list(continue_listening: &[Root]) -> Vec<String> {
    let mut pub_year_cnt_list = Vec::new();

    for library in continue_listening {
        if let Some(entities) = &library.entities {
            for entity in entities {
                let published_year = entity.media.as_ref()
                    .and_then(|media| media.metadata.as_ref())
                    .and_then(|metadata| metadata.published_year.clone());
                pub_year_cnt_list.push(published_year.unwrap_or_else(|| "N/A".to_string()));
            }
        }
    }

    pub_year_cnt_list
}

/// collect duration - one entry per entity, see above.
pub async fn collect_duration_cnt_list(continue_listening: &[Root]) -> Vec<f64> {

    let mut duration_cnt_list = vec![];

    for library in continue_listening {
        if let Some(entities) = &library.entities {
            for entity in entities {
                let duration = entity.media.as_ref().and_then(|media| media.duration);
                duration_cnt_list.push(duration.unwrap_or(0.0));
            }
        }
    }

    duration_cnt_list

}

/// collect file size (bytes) - one entry per entity, see above.
pub async fn collect_size_cnt_list(continue_listening: &[Root]) -> Vec<i64> {

    let mut size_cnt_list = vec![];

    for library in continue_listening {
        if let Some(entities) = &library.entities {
            for entity in entities {
                let size = entity.media.as_ref().and_then(|media| media.size);
                size_cnt_list.push(size.unwrap_or(0));
            }
        }
    }

    size_cnt_list

}

/// collect description - one entry per entity, see above.
pub async fn collect_desc_cnt_list(continue_listening: &[Root]) -> Vec<String> {
    let mut desc_cnt_list = Vec::new();

    for library in continue_listening {
        if let Some(entities) = &library.entities {
            for entity in entities {
                let description = entity.media.as_ref()
                    .and_then(|media| media.metadata.as_ref())
                    .and_then(|metadata| metadata.description.clone());
                desc_cnt_list.push(description.unwrap_or_else(|| "N/A".to_string()));
            }
        }
    }

    desc_cnt_list
}

/// collect ID of the library item
pub async fn collect_ids_cnt_list(continue_listening: &[Root]) -> Vec<String> {
    let mut ids_cnt_list = Vec::new();  

    for library in continue_listening {
        if let Some(entities) = &library.entities {
            for entity in entities {
                if let Some(id) = &entity.id { 
                    ids_cnt_list.push(id.clone()); 
                } else {
                    ids_cnt_list.push("N/A".to_string());
                }

            }
        }
    }

    ids_cnt_list 
}
