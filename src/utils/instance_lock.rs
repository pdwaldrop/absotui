use std::env;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// Every running Absotui holds a shared lock on this file, so one that can take it
// exclusively knows it is alone. The OS drops the lock when a process dies, crash included.
static OWN_LOCK: Mutex<Option<File>> = Mutex::new(None);

fn lock_path() -> PathBuf {
    let config_home = env::var("XDG_CONFIG_HOME").map_or_else(|_| {
        let mut path = dirs::home_dir().expect("Unable to find the user's home directory");
        if cfg!(target_os = "macos") {
            path.push("Library/Preferences");
        } else {
            path.push(".config");
        }
        path
    }, PathBuf::from);
    config_home.join("absotui/instance.lock")
}

fn open(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path)
}

fn others_running(path: &Path) -> bool {
    let Ok(file) = open(path) else { return false };
    match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            false
        }
        Err(TryLockError::WouldBlock) => true,
        Err(TryLockError::Error(_)) => false,
    }
}

fn register_at(path: &Path) -> bool {
    let others = others_running(path);
    if let Ok(file) = open(path)
        && file.lock_shared().is_ok() {
            *OWN_LOCK.lock().unwrap_or_else(|e| e.into_inner()) = Some(file);
    }
    others
}

/// Marks this process as running; returns whether another Absotui already was.
pub fn register() -> bool {
    register_at(&lock_path())
}

/// Gives up this process's mark (it is exiting) and returns whether another Absotui
/// is still running.
pub fn others_running_at_exit() -> bool {
    OWN_LOCK.lock().unwrap_or_else(|e| e.into_inner()).take();
    others_running(&lock_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_lock(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("absotui-instance-lock-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("instance.lock")
    }

    #[test]
    fn nobody_else_running_when_no_one_holds_the_lock() {
        let path = scratch_lock("alone");
        assert!(!others_running(&path));
    }

    #[test]
    fn another_instance_holding_a_shared_lock_is_seen() {
        let path = scratch_lock("other");
        let other = open(&path).unwrap();
        other.lock_shared().unwrap();
        assert!(others_running(&path));
        drop(other);
        assert!(!others_running(&path));
    }

    #[test]
    fn a_registered_instance_is_seen_until_it_lets_go() {
        let path = scratch_lock("registered");
        assert!(!register_at(&path));
        assert!(others_running(&path));
        OWN_LOCK.lock().unwrap().take();
        assert!(!others_running(&path));
    }

    #[test]
    fn the_probe_leaves_no_lock_behind() {
        let path = scratch_lock("probe");
        assert!(!others_running(&path));
        let next = open(&path).unwrap();
        assert!(next.try_lock().is_ok());
    }
}
