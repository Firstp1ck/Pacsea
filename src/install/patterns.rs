/*!
Pattern configuration loader for the custom suspicious-patterns scan.

Purpose:
- Allow users to tune suspicious pattern categories via a simple config file:
  $XDG_CONFIG_HOME/pacsea/pattern.conf (or $HOME/.config/pacsea/pattern.conf)

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

use std::fs;
use std::path::PathBuf;

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

impl Default for PatternSets {
    fn default() -> Self {
        // Defaults intentionally mirror the scanner's built-in bash ERE sets.
        // These are intended for grep -E (ERE) within bash, not Rust regex compilation.
        let critical = r#"(/dev/(tcp|udp)/|bash -i *>& *[^ ]*/dev/(tcp|udp)/[0-9]+|exec [0-9]{2,}<>/dev/(tcp|udp)/|rm -rf[[:space:]]+/|dd if=/dev/zero of=/dev/sd[a-z]|[>]{1,2}[[:space:]]*/dev/sd[a-z]|: *\(\) *\{ *: *\| *: *& *\};:|/etc/sudoers([[:space:]>]|$)|echo .*[>]{2}.*(/etc/sudoers|/root/.ssh/authorized_keys)|/etc/ld\.so\.preload|LD_PRELOAD=|authorized_keys.*[>]{2}|ssh-rsa [A-Za-z0-9+/=]+.*[>]{2}.*authorized_keys|curl .*(169\.254\.169\.254))"#.to_string();

        let high = r#"(eval|base64 -d|wget .*(sh|bash|dash|ksh|zsh)([^A-Za-z]|$)|curl .*(sh|bash|dash|ksh|zsh)([^A-Za-z]|$)|sudo[[:space:]]|chattr[[:space:]]|useradd|adduser|groupadd|systemctl|service[[:space:]]|crontab|/etc/cron\.|[>]{2}.*(\.bashrc|\.bash_profile|/etc/profile|\.zshrc)|cat[[:space:]]+/etc/shadow|cat[[:space:]]+~/.ssh/id_rsa|cat[[:space:]]+~/.bash_history|systemctl stop (auditd|rsyslog)|service (auditd|rsyslog) stop|scp .*@|curl -F|nc[[:space:]].*<|tar -czv?f|zip -r)"#.to_string();

        let medium = r#"(whoami|uname -a|hostname|id|groups|nmap|netstat -anp|ss -anp|ifconfig|ip addr|arp -a|grep -ri .*secret|find .*-name.*(password|\.key)|env[[:space:]]*\|[[:space:]]*grep -i pass|wget https?://|curl https?://)"#.to_string();

        let low = r#"(http_proxy=|https_proxy=|ALL_PROXY=|yes[[:space:]]+> */dev/null *&|ulimit -n [0-9]{5,})"#.to_string();

        Self {
            critical,
            high,
            medium,
            low,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Section {
    Critical,
    High,
    Medium,
    Low,
}

/// Attempt to load pattern sets from $XDG_CONFIG_HOME/pacsea/pattern.conf
/// (falling back to $HOME/.config/pacsea/pattern.conf). Returns defaults
/// when missing or on parse errors.
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

/// Return the canonical pattern.conf path under Pacsea's config dir.
fn config_path() -> PathBuf {
    crate::theme::config_dir().join("pattern.conf")
}

/// Parse pattern.conf content; section lines are joined with `|`.
/// Comments: lines starting with '#', '//' or ';'. Empty lines ignored.
/// Any section missing or empty falls back to the current defaults provided.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_uses_defaults_when_empty() {
        let d = PatternSets::default();
        let p = parse("", &d);
        assert_eq!(p.critical, d.critical);
        assert_eq!(p.high, d.high);
        assert_eq!(p.medium, d.medium);
        assert_eq!(p.low, d.low);
    }

    #[test]
    fn parse_joins_lines_with_or() {
        let d = PatternSets::default();
        let cfg = r#"
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
        "#;
        let p = parse(cfg, &d);
        assert_eq!(p.critical, "a|b|c");
        assert_eq!(p.high, "foo|bar");
        assert_eq!(p.medium, "x");
        assert_eq!(p.low, "l1|l2");
    }

    #[test]
    fn parse_handles_comments_and_whitespace() {
        let d = PatternSets::default();
        let cfg = r#"
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
        "#;
        let p = parse(cfg, &d);
        assert_eq!(p.critical, "a|b");
        assert_eq!(p.high, "foo");
        assert_eq!(p.medium, d.medium);
        assert_eq!(p.low, "l1");
    }
}
