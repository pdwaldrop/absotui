use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

const OWNER_ONLY: u32 = 0o600;

/// Tightens an existing file to owner-only if group or others can access it. A missing
/// file is left alone.
pub fn restrict_existing(path: &Path) {
    if let Ok(meta) = fs::metadata(path)
        && meta.is_file()
        && meta.permissions().mode() & 0o077 != 0
    {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(OWNER_ONLY));
    }
}

/// Creates the file owner-only when it doesn't exist yet (an empty file is a valid new
/// SQLite database), otherwise tightens it. Failures are left for the caller's own open
/// to report.
pub fn create_private_or_restrict(path: &Path) {
    match OpenOptions::new().write(true).create_new(true).mode(OWNER_ONLY).open(path) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => restrict_existing(path),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("absotui-private-files-{}-{name}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn restrict_existing_tightens_a_world_readable_file_and_keeps_its_content() {
        let dir = scratch_dir("tighten");
        let file = dir.join(".env");
        fs::write(&file, "ABSOTUI_SECRET_KEY=abc").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
        restrict_existing(&file);
        assert_eq!(mode(&file), 0o600);
        assert_eq!(fs::read_to_string(&file).unwrap(), "ABSOTUI_SECRET_KEY=abc");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restrict_existing_leaves_a_file_with_no_group_or_other_access_alone() {
        let dir = scratch_dir("owner-only");
        let file = dir.join("db.sqlite3");
        fs::write(&file, "x").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o400)).unwrap();
        restrict_existing(&file);
        assert_eq!(mode(&file), 0o400);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn restrict_existing_does_not_create_a_missing_file() {
        let dir = scratch_dir("missing");
        let file = dir.join(".env");
        restrict_existing(&file);
        assert!(!file.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_private_or_restrict_creates_an_owner_only_file() {
        let dir = scratch_dir("create");
        let file = dir.join("db.sqlite3");
        create_private_or_restrict(&file);
        assert!(file.exists());
        assert_eq!(mode(&file), 0o600);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_private_or_restrict_tightens_an_existing_file_without_truncating_it() {
        let dir = scratch_dir("existing");
        let file = dir.join("db.sqlite3");
        fs::write(&file, "existing data").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o666)).unwrap();
        create_private_or_restrict(&file);
        assert_eq!(mode(&file), 0o600);
        assert_eq!(fs::read_to_string(&file).unwrap(), "existing data");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sqlite_accepts_the_empty_pre_created_file_as_a_new_database() {
        let dir = scratch_dir("sqlite");
        let file = dir.join("db.sqlite3");
        create_private_or_restrict(&file);
        let conn = rusqlite::Connection::open(&file).unwrap();
        conn.execute("CREATE TABLE t (a TEXT)", []).unwrap();
        conn.execute("INSERT INTO t VALUES ('x')", []).unwrap();
        drop(conn);
        assert_eq!(mode(&file), 0o600);
        let _ = fs::remove_dir_all(&dir);
    }
}
