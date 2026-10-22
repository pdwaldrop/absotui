use log::LevelFilter;
use fern::Dispatch;
use chrono::Local;
use std::fs::OpenOptions;
use std::env;
use std::path::{Path, PathBuf};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

// Keeps one previous log (`absotui.log.old`) so a crash report still has context, while
// the log can't grow without bound.
fn rotate_if_large(log_path: &Path, max_bytes: u64) {
    let too_big = std::fs::metadata(log_path).is_ok_and(|m| m.len() > max_bytes);
    if too_big {
        let _ = std::fs::rename(log_path, log_path.with_extension("log.old"));
    }
}

pub fn setup_logs() -> Result<(), fern::InitError> {

    let config_home_path = env::var("XDG_CONFIG_HOME").map_or_else(|_| { 
            let mut path = dirs::home_dir().expect("Unable to find the user's home directory");

            if cfg!(target_os = "macos") {
                path.push("Library/Preferences");
            } else {
                path.push(".config");
            }

            path
        }, PathBuf::from);

    let log_path = config_home_path.join("absotui/absotui.log");

    // `OpenOptions::create(true)` creates the file but not its parent directory - on a
    // genuinely fresh environment (built from source and run directly, or a
    // not-yet-created XDG_CONFIG_HOME override) this is the very first fallible thing
    // main() does, so a missing ~/.config/absotui/ would otherwise panic with a raw,
    // unhelpful IO error before anything else has a chance to run.
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    rotate_if_large(&log_path, MAX_LOG_BYTES);

    // Create or append into the file
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?; // path and name

    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                    "{} [{}] - {}",
                    Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                    record.level(),
                    message
            ));
        })
    .level(LevelFilter::Info) 
        .chain(log_file) // redirect logs to the file 
        .apply()?; 

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("absotui-logs-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_oversized_log_is_moved_aside() {
        let dir = scratch_dir("big");
        let log = dir.join("absotui.log");
        std::fs::write(&log, vec![b'x'; 2048]).unwrap();
        rotate_if_large(&log, 1024);
        assert!(!log.exists());
        assert_eq!(std::fs::metadata(dir.join("absotui.log.old")).unwrap().len(), 2048);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_small_or_missing_log_is_left_alone() {
        let dir = scratch_dir("small");
        let log = dir.join("absotui.log");
        rotate_if_large(&log, 1024);
        std::fs::write(&log, b"short").unwrap();
        rotate_if_large(&log, 1024);
        assert!(log.exists());
        assert!(!dir.join("absotui.log.old").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
