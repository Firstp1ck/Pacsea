/*!
Pattern configuration loader for the custom suspicious-patterns scan.

Purpose:
- Allow users to tune suspicious pattern categories via a simple config file:
  `$XDG_CONFIG_HOME`/pacsea/pattern.conf (or `$HOME`/.config/pacsea/pattern.conf)

Format:
- INI-like sections: [critical], [high], [medium], [low]
- Each non-empty, non-comment line within a section is treated as a raw ERE (Extended Regex)
  fragment (compatible with `grep -E`). At runtime, all lines in a section are joined with `|`.
- Comments start with '#', '//' or ';'. Empty lines are ignored.

Example pattern.conf:

```ini
# Customize suspicious patterns (ERE fragments)
[critical]
/dev/(tcp|udp)/
rm -rf[[:space:]]+/
: *\(\) *\{ *: *\| *: *& *\};:
/etc/sudoers([[:space:]>]|$)

[high]
eval
base64 -d
wget .*(sh|bash)([^A-Za-z]|$)
curl .*(sh|bash)([^A-Za-z]|$)

[medium]
whoami
uname -a
grep -ri .*secret

[low]
http_proxy=
https_proxy=
```

Notes:
- This loader returns joined strings for each category. The scanner shells them into `grep -Eo`.
- Defaults are chosen to mirror built-in patterns used by the scan pipeline.
*/

#[cfg(not(target_os = "windows"))]
use std::fs;
#[cfg(not(target_os = "windows"))]
use std::path::PathBuf;

#[cfg(not(target_os = "windows"))]
/// Grouped suspicious pattern sets (ERE fragments joined by `|`).
#[derive(Clone, Debug)]
pub struct PatternSets {
    /// Critical-severity indicators. High-confidence red flags.
    pub critical: String,
    /// High-severity indicators. Strong suspicious behaviors.
    pub high: String,
    /// Medium-severity indicators. Recon/sensitive searches and downloads.
    pub medium: String,
    /// Low-severity indicators. Environment hints/noise.
    pub low: String,
}

#[cfg(not(target_os = "windows"))]
impl Default for PatternSets {
    fn default() -> Self {
        // Defaults intentionally mirror the scanner's built-in bash ERE sets.
        // These are intended for grep -E (ERE) within bash, not Rust regex compilation.
        let critical = r"(/dev/(tcp|udp)/|bash -i *>& *[^ ]*/dev/(tcp|udp)/[0-9]+|exec [0-9]{2,}<>/dev/(tcp|udp)/|rm -rf[[:space:]]+/|dd if=/dev/zero of=/dev/sd[a-z]|[>]{1,2}[[:space:]]*/dev/sd[a-z]|: *\(\) *\{ *: *\| *: *& *\};:|/etc/sudoers([[:space:]>]|$)|echo .*[>]{2}.*(/etc/sudoers|/root/.ssh/authorized_keys)|/etc/ld\.so\.preload|LD_PRELOAD=|authorized_keys.*[>]{2}|ssh-rsa [A-Za-z0-9+/=]+.*[>]{2}.*authorized_keys|curl .*(169\.254\.169\.254))".to_string();

        let high = r"(eval|base64 -d|wget .*(sh|bash|dash|ksh|zsh)([^A-Za-z]|$)|curl .*(sh|bash|dash|ksh|zsh)([^A-Za-z]|$)|sudo[[:space:]]|chattr[[:space:]]|useradd|adduser|groupadd|systemctl|service[[:space:]]|crontab|/etc/cron\.|[>]{2}.*(\.bashrc|\.bash_profile|/etc/profile|\.zshrc)|cat[[:space:]]+/etc/shadow|cat[[:space:]]+~/.ssh/id_rsa|cat[[:space:]]+~/.bash_history|systemctl stop (auditd|rsyslog)|service (auditd|rsyslog) stop|scp .*@|curl -F|nc[[:space:]].*<|tar -czv?f|zip -r)".to_string();

        let medium = r"(whoami|uname -a|hostname|id|groups|nmap|netstat -anp|ss -anp|ifconfig|ip addr|arp -a|grep -ri .*secret|find .*-name.*(password|\.key)|env[[:space:]]*\|[[:space:]]*grep -i pass|wget https?://|curl https?://)".to_string();

        let low = r"(http_proxy=|https_proxy=|ALL_PROXY=|yes[[:space:]]+> */dev/null *&|ulimit -n [0-9]{5,})".to_string();

        Self {
            critical,
            high,
            medium,
            low,
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Section {
    Critical,
    High,
    Medium,
    Low,
}

#[cfg(not(target_os = "windows"))]
/// What: Load suspicious pattern sets from the user's `pattern.conf`.
///
/// Input:
/// - Reads `$XDG_CONFIG_HOME/pacsea/pattern.conf` (falling back to `$HOME/.config/pacsea/pattern.conf`).
///
/// Output:
/// - `PatternSets` containing joined regex fragments for each severity bucket.
///
/// Details:
/// - Falls back to built-in defaults when the file is missing or malformed.
/// - Uses simple INI-style parsing, ignoring unknown sections and comments.
pub fn load() -> PatternSets {
    let mut out = PatternSets::default();
    let path = config_path();

    match fs::read_to_string(&path) {
        Ok(content) => {
            let parsed = parse(&content, &out);
            out = parsed;
        }
        Err(_) => {
            // Keep defaults when missing/unreadable
        }
    }
    out
}

#[cfg(not(target_os = "windows"))]
/// What: Resolve the canonical location of `pattern.conf` in the Pacsea config directory.
///
/// Input:
/// - None (derives the path from Pacsea's configuration base).
///
/// Output:
/// - Absolute `PathBuf` pointing to `pattern.conf`.
///
/// Details:
/// - Relies on `crate::theme::config_dir()` to honour XDG overrides.
fn config_path() -> PathBuf {
    crate::theme::config_dir().join("pattern.conf")
}

#[cfg(not(target_os = "windows"))]
/// What: Parse raw `pattern.conf` content into severity buckets.
///
/// Input:
/// - `content`: File body to parse.
/// - `defaults`: Existing `PatternSets` used when a section is absent or empty.
///
/// Output:
/// - `PatternSets` with each section joined by `|`.
///
/// Details:
/// - Treats lines beginning with `#`, `//`, or `;` as comments.
/// - Recognises `[critical]`, `[high]`, `[medium]`, and `[low]` sections (case-insensitive aliases allowed).
/// - Unrecognised sections are ignored without error.
fn parse(content: &str, defaults: &PatternSets) -> PatternSets {
    use Section::*;

    let mut cur: Option<Section> = None;

    let mut c: Vec<String> = Vec::new();
    let mut h: Vec<String> = Vec::new();
    let mut m: Vec<String> = Vec::new();
    let mut l: Vec<String> = Vec::new();

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("//")
            || line.starts_with(';')
        {
            continue;
        }
        if line.starts_with('[')
            && let Some(end) = line.find(']')
        {
            let name = line[1..end].to_ascii_lowercase();
            cur = match name.as_str() {
                "critical" | "crit" => Some(Critical),
                "high" | "hi" => Some(High),
                "medium" | "med" => Some(Medium),
                "low" => Some(Low),
                _ => None,
            };
            continue;
        }
        if let Some(sec) = cur {
            // Store raw ERE fragments for later `|` join
            match sec {
                Critical => c.push(line.to_string()),
                High => h.push(line.to_string()),
                Medium => m.push(line.to_string()),
                Low => l.push(line.to_string()),
            }
        }
    }

    let critical = if c.is_empty() {
        defaults.critical.clone()
    } else {
        c.join("|")
    };
    let high = if h.is_empty() {
        defaults.high.clone()
    } else {
        h.join("|")
    };
    let medium = if m.is_empty() {
        defaults.medium.clone()
    } else {
        m.join("|")
    };
    let low = if l.is_empty() {
        defaults.low.clone()
    } else {
        l.join("|")
    };

    PatternSets {
        critical,
        high,
        medium,
        low,
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;

    #[test]
    /// What: Ensure `load` falls back to defaults when no pattern configuration file exists.
    ///
    /// Input:
    /// - Temporary HOME without an accompanying `pattern.conf`.
    ///
    /// Output:
    /// - Loaded pattern sets match `PatternSets::default`.
    ///
    /// Details:
    /// - Redirects `HOME`, guards with the theme mutex, and removes the temp directory after assertions.
    fn load_returns_defaults_when_config_missing() {
        use std::fs;
        use std::path::PathBuf;
        let _guard = crate::theme::test_mutex()
            .lock()
            .expect("Test mutex poisoned");

        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_patterns_load_missing_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);

        let orig_home = std::env::var_os("HOME");
        let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
        unsafe {
            std::env::set_var("HOME", dir.display().to_string());
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        let defaults = PatternSets::default();
        let loaded = super::load();
        assert_eq!(loaded.critical, defaults.critical);
        assert_eq!(loaded.high, defaults.high);
        assert_eq!(loaded.medium, defaults.medium);
        assert_eq!(loaded.low, defaults.low);

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
            if let Some(v) = orig_xdg {
                std::env::set_var("XDG_CONFIG_HOME", v);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    /// What: Ensure `load` honours pattern definitions from an on-disk configuration file.
    ///
    /// Input:
    /// - Temporary HOME containing a handwritten `pattern.conf` with custom sections.
    ///
    /// Output:
    /// - Loaded pattern sets reflect the configured critical/high/medium/low regexes.
    ///
    /// Details:
    /// - Writes `pattern.conf` under Pacsea's config directory, then restores environment variables and removes artifacts.
    fn load_reads_pattern_conf_overrides() {
        use std::fs;
        use std::path::PathBuf;
        let _guard = crate::theme::test_mutex()
            .lock()
            .expect("Test mutex poisoned");

        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_patterns_load_conf_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);

        let orig_home = std::env::var_os("HOME");
        let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
        unsafe {
            std::env::set_var("HOME", dir.display().to_string());
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        let config_dir = crate::theme::config_dir();
        let pattern_path = config_dir.join("pattern.conf");
        let body = "[critical]\nfoo\n\n[high]\nbar\n\n[medium]\nmid\n\n[low]\nlo\n";
        fs::write(&pattern_path, body).expect("failed to write test pattern config file");

        let loaded = super::load();
        assert_eq!(loaded.critical, "foo");
        assert_eq!(loaded.high, "bar");
        assert_eq!(loaded.medium, "mid");
        assert_eq!(loaded.low, "lo");

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
            if let Some(v) = orig_xdg {
                std::env::set_var("XDG_CONFIG_HOME", v);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    /// What: Confirm `parse` falls back to default regex sets when the config snippet is empty.
    ///
    /// Inputs:
    /// - Blank configuration string.
    /// - `PatternSets::default()` as the baseline values.
    ///
    /// Output:
    /// - Returns a `PatternSets` identical to the defaults.
    ///
    /// Details:
    /// - Exercises the early-return path that clones defaults for each severity bucket.
    fn parse_uses_defaults_when_empty() {
        let d = PatternSets::default();
        let p = parse("", &d);
        assert_eq!(p.critical, d.critical);
        assert_eq!(p.high, d.high);
        assert_eq!(p.medium, d.medium);
        assert_eq!(p.low, d.low);
    }

    #[test]
    /// What: Ensure `parse` concatenates multi-line sections with `|` to form extended regexes.
    ///
    /// Inputs:
    /// - Config snippet containing multiple severities with repeated entries.
    ///
    /// Output:
    /// - Generated pattern strings join entries with `|` while preserving singleton sections.
    ///
    /// Details:
    /// - Verifies each severity bucket independently to catch regressions in join order.
    fn parse_joins_lines_with_or() {
        let d = PatternSets::default();
        let cfg = r"
            [critical]
            a
            b
            c

            [high]
            foo
            bar

            [medium]
            x

            [low]
            l1
            l2
        ";
        let p = parse(cfg, &d);
        assert_eq!(p.critical, "a|b|c");
        assert_eq!(p.high, "foo|bar");
        assert_eq!(p.medium, "x");
        assert_eq!(p.low, "l1|l2");
    }

    #[test]
    /// What: Verify `parse` ignores comments, unknown sections, and insignificant whitespace.
    ///
    /// Inputs:
    /// - Config snippet with comment prefixes (`#`, `;`, `//`), extra indentation, and an unknown header.
    ///
    /// Output:
    /// - Patterns exclude commented lines, skip the unknown section, and trim whitespace in recognised sections.
    ///
    /// Details:
    /// - Confirms default fallback remains for untouched severities while demonstrating indentation trimming for `low`.
    fn parse_handles_comments_and_whitespace() {
        let d = PatternSets::default();
        let cfg = r"
            # comment
            ; also comment
            // yet another

            [critical]
            a
            #ignored
            b

            [unknown]    # ignored section (no effect)

            [high]
            foo

            [low]
                l1
            ";
        let p = parse(cfg, &d);
        assert_eq!(p.critical, "a|b");
        assert_eq!(p.high, "foo");
        assert_eq!(p.medium, d.medium);
        assert_eq!(p.low, "l1");
    }
}
