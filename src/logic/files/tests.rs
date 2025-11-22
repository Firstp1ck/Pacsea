//! Tests for file resolution and parsing functions.

use super::pkgbuild_parse::{
    parse_backup_array_content, parse_backup_from_pkgbuild, parse_backup_from_srcinfo,
};
use super::resolution::{resolve_install_files, resolve_remove_files};
use crate::state::modal::FileChangeType;
use crate::state::types::Source;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

struct PathGuard {
    original: Option<String>,
}

impl PathGuard {
    fn push(dir: &std::path::Path) -> Self {
        let original = std::env::var("PATH").ok();
        // If PATH is missing or empty, use a default system PATH
        let base_path = original
            .as_ref()
            .filter(|p| !p.is_empty())
            .map(|s| s.as_str())
            .unwrap_or("/usr/bin:/bin:/usr/local/bin");
        let mut new_path = dir.display().to_string();
        new_path.push(':');
        new_path.push_str(base_path);
        unsafe {
            std::env::set_var("PATH", &new_path);
        }
        Self { original }
    }
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        if let Some(ref orig) = self.original {
            // Only restore if the original PATH was valid (not empty)
            if !orig.is_empty() {
                unsafe {
                    std::env::set_var("PATH", orig);
                }
            } else {
                // If original was empty, restore to a default system PATH
                unsafe {
                    std::env::set_var("PATH", "/usr/bin:/bin:/usr/local/bin");
                }
            }
        } else {
            // If PATH was missing, set a default system PATH
            unsafe {
                std::env::set_var("PATH", "/usr/bin:/bin:/usr/local/bin");
            }
        }
    }
}

fn write_executable(dir: &std::path::Path, name: &str, body: &str) {
    let path = dir.join(name);
    let mut file = fs::File::create(&path).expect("create stub");
    file.write_all(body.as_bytes()).expect("write stub");
    let mut perms = fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).expect("chmod stub");
}

#[test]
fn test_parse_backup_from_pkgbuild_single_line() {
    let pkgbuild = r"
pkgname=test
pkgver=1.0
backup=('/etc/config' '/etc/other.conf')
";
    let backup_files = parse_backup_from_pkgbuild(pkgbuild);
    assert_eq!(backup_files.len(), 2);
    assert!(backup_files.contains(&"/etc/config".to_string()));
    assert!(backup_files.contains(&"/etc/other.conf".to_string()));
}

#[test]
fn test_parse_backup_from_pkgbuild_multi_line() {
    let pkgbuild = r"
pkgname=test
pkgver=1.0
backup=(
    '/etc/config'
    '/etc/other.conf'
    '/etc/more.conf'
)
";
    let backup_files = parse_backup_from_pkgbuild(pkgbuild);
    assert_eq!(backup_files.len(), 3);
    assert!(backup_files.contains(&"/etc/config".to_string()));
    assert!(backup_files.contains(&"/etc/other.conf".to_string()));
    assert!(backup_files.contains(&"/etc/more.conf".to_string()));
}

#[test]
fn test_parse_backup_from_srcinfo() {
    let srcinfo = r"
pkgbase = test-package
pkgname = test-package
pkgver = 1.0.0
backup = /etc/config
backup = /etc/other.conf
backup = /etc/more.conf
";
    let backup_files = parse_backup_from_srcinfo(srcinfo);
    assert_eq!(backup_files.len(), 3);
    assert!(backup_files.contains(&"/etc/config".to_string()));
    assert!(backup_files.contains(&"/etc/other.conf".to_string()));
    assert!(backup_files.contains(&"/etc/more.conf".to_string()));
}

#[test]
fn test_parse_backup_array_content() {
    let content = "'/etc/config' '/etc/other.conf'";
    let mut backup_files = Vec::new();
    parse_backup_array_content(content, &mut backup_files);
    assert_eq!(backup_files.len(), 2);
    assert!(backup_files.contains(&"/etc/config".to_string()));
    assert!(backup_files.contains(&"/etc/other.conf".to_string()));
}

#[test]
/// What: Resolve install file information using stubbed pacman output while verifying pacnew detection.
///
/// Inputs:
/// - Stub `pacman` script returning canned `-Fl`, `-Ql`, and `-Qii` outputs for package `pkg`.
///
/// Output:
/// - `resolve_install_files` reports one changed config file and one new regular file with pacnew prediction.
///
/// Details:
/// - Uses a temporary PATH override and the global test mutex to isolate command stubbing from other tests.
fn resolve_install_files_marks_changed_and_new_entries() {
    let _test_guard = crate::global_test_mutex_lock();
    // Ensure PATH is in a clean state before modifying it
    if std::env::var("PATH").is_err() {
        unsafe { std::env::set_var("PATH", "/usr/bin:/bin:/usr/local/bin") };
    }
    let dir = tempdir().expect("tempdir");
    let _path_guard = PathGuard::push(dir.path());
    write_executable(
        dir.path(),
        "pacman",
        r#"#!/bin/sh
if [ "$1" = "-Fl" ]; then
cat <<'EOF'
pkg /etc/app.conf
pkg /usr/share/doc/
pkg /usr/bin/newtool
EOF
exit 0
fi
if [ "$1" = "-Ql" ]; then
cat <<'EOF'
pkg /etc/app.conf
EOF
exit 0
fi
if [ "$1" = "-Qii" ]; then
cat <<'EOF'
Backup Files  : /etc/app.conf
EOF
exit 0
fi
if [ "$1" = "-Fy" ]; then
exit 0
fi
exit 1
"#,
    );

    let source = Source::Official {
        repo: "core".into(),
        arch: "x86_64".into(),
    };
    let info = resolve_install_files("pkg", &source).expect("install resolution");

    assert_eq!(info.total_count, 2);
    assert_eq!(info.new_count, 1);
    assert_eq!(info.changed_count, 1);
    assert_eq!(info.config_count, 1);
    assert_eq!(info.pacnew_candidates, 1);

    let mut paths: Vec<&str> = info.files.iter().map(|f| f.path.as_str()).collect();
    paths.sort();
    assert_eq!(paths, vec!["/etc/app.conf", "/usr/bin/newtool"]);

    let config_entry = info
        .files
        .iter()
        .find(|f| f.path == "/etc/app.conf")
        .expect("config entry");
    assert!(matches!(config_entry.change_type, FileChangeType::Changed));
    assert!(config_entry.predicted_pacnew);
    assert!(!config_entry.predicted_pacsave);

    let new_entry = info
        .files
        .iter()
        .find(|f| f.path == "/usr/bin/newtool")
        .expect("new entry");
    assert!(matches!(new_entry.change_type, FileChangeType::New));
    assert!(!new_entry.predicted_pacnew);
}

#[test]
/// What: Resolve removal file information with stubbed pacman output to confirm pacsave predictions.
///
/// Inputs:
/// - Stub `pacman` script returning canned `-Ql` and `-Qii` outputs listing a config and regular file.
///
/// Output:
/// - `resolve_remove_files` reports both files as removed while flagging the config as a pacsave candidate.
///
/// Details:
/// - Shares the PATH guard helper to ensure the stubbed command remains isolated per test.
fn resolve_remove_files_marks_pacsave_candidates() {
    let _test_guard = crate::global_test_mutex_lock();
    // Ensure PATH is in a clean state before modifying it
    if std::env::var("PATH").is_err() {
        unsafe { std::env::set_var("PATH", "/usr/bin:/bin:/usr/local/bin") };
    }
    let dir = tempdir().expect("tempdir");
    let _path_guard = PathGuard::push(dir.path());
    write_executable(
        dir.path(),
        "pacman",
        r#"#!/bin/sh
if [ "$1" = "-Ql" ]; then
cat <<'EOF'
pkg /etc/app.conf
pkg /usr/bin/newtool
EOF
exit 0
fi
if [ "$1" = "-Qii" ]; then
cat <<'EOF'
Backup Files  : /etc/app.conf
EOF
exit 0
fi
if [ "$1" = "-Fy" ] || [ "$1" = "-Fl" ]; then
exit 0
fi
exit 1
"#,
    );

    let info = resolve_remove_files("pkg").expect("remove resolution");

    assert_eq!(info.removed_count, 2);
    assert_eq!(info.config_count, 1);
    assert_eq!(info.pacsave_candidates, 1);

    let config_entry = info
        .files
        .iter()
        .find(|f| f.path == "/etc/app.conf")
        .expect("config entry");
    assert!(config_entry.is_config);
    assert!(config_entry.predicted_pacsave);
    assert!(!config_entry.predicted_pacnew);

    let regular_entry = info
        .files
        .iter()
        .find(|f| f.path == "/usr/bin/newtool")
        .expect("regular entry");
    assert!(!regular_entry.is_config);
    assert!(!regular_entry.predicted_pacsave);
}
