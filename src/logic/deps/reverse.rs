//! Reverse dependency analysis for removal preflight checks.

use crate::state::modal::{DependencyInfo, DependencySource, DependencyStatus, ReverseRootSummary};
use crate::state::types::PackageItem;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque, hash_map::Entry};
use std::process::Command;

/// Result bundle from reverse dependency resolution.
#[derive(Debug, Default)]
pub struct ReverseDependencyReport {
    /// Flattened dependency info reused by the Preflight modal UI.
    pub dependencies: Vec<DependencyInfo>,
    /// Per-root summary statistics for the Summary tab.
    pub summaries: Vec<ReverseRootSummary>,
}

struct ReverseResolverState {
    aggregated: HashMap<String, AggregatedEntry>,
    cache: HashMap<String, PkgInfo>,
    missing: HashSet<String>,
    target_names: HashSet<String>,
}

impl ReverseResolverState {
    fn new(targets: &[PackageItem]) -> Self {
        let target_names = targets.iter().map(|pkg| pkg.name.clone()).collect();
        Self {
            aggregated: HashMap::new(),
            cache: HashMap::new(),
            missing: HashSet::new(),
            target_names,
        }
    }

    fn pkg_info(&mut self, name: &str) -> Option<PkgInfo> {
        if let Some(info) = self.cache.get(name) {
            return Some(info.clone());
        }
        if self.missing.contains(name) {
            return None;
        }

        match fetch_pkg_info(name) {
            Ok(info) => {
                self.cache.insert(name.to_string(), info.clone());
                Some(info)
            }
            Err(err) => {
                tracing::warn!("Failed to query pacman -Qi {}: {}", name, err);
                self.missing.insert(name.to_string());
                None
            }
        }
    }

    fn update_entry(&mut self, dependent: &str, parent: &str, root: &str, depth: usize) {
        if dependent.eq_ignore_ascii_case(root) {
            return;
        }

        let Some(info) = self.pkg_info(dependent) else {
            return;
        };

        let selected = self.target_names.contains(dependent);
        match self.aggregated.entry(dependent.to_string()) {
            Entry::Occupied(mut entry) => {
                let data = entry.get_mut();
                data.info = info;
                if selected {
                    data.selected_for_removal = true;
                }
                let relation = data
                    .per_root
                    .entry(root.to_string())
                    .or_insert_with(RootRelation::new);
                relation.record(parent, depth);
            }
            Entry::Vacant(slot) => {
                let mut data = AggregatedEntry {
                    info,
                    per_root: HashMap::new(),
                    selected_for_removal: selected,
                };
                data.per_root
                    .entry(root.to_string())
                    .or_insert_with(RootRelation::new)
                    .record(parent, depth);
                slot.insert(data);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct PkgInfo {
    name: String,
    version: String,
    repo: Option<String>,
    groups: Vec<String>,
    required_by: Vec<String>,
    explicit: bool,
}

#[derive(Clone, Debug)]
struct AggregatedEntry {
    info: PkgInfo,
    per_root: HashMap<String, RootRelation>,
    selected_for_removal: bool,
}

#[derive(Clone, Debug)]
struct RootRelation {
    parents: HashSet<String>,
    min_depth: usize,
}

impl RootRelation {
    fn new() -> Self {
        Self {
            parents: HashSet::new(),
            min_depth: usize::MAX,
        }
    }

    fn record(&mut self, parent: &str, depth: usize) {
        if !parent.is_empty() {
            self.parents.insert(parent.to_string());
        }
        if depth < self.min_depth {
            self.min_depth = depth;
        }
    }

    fn min_depth(&self) -> usize {
        self.min_depth
    }
}

/// Resolve reverse dependencies for packages slated for removal.
pub fn resolve_reverse_dependencies(targets: &[PackageItem]) -> ReverseDependencyReport {
    tracing::info!(
        "Starting reverse dependency resolution for {} target(s)",
        targets.len()
    );

    if targets.is_empty() {
        return ReverseDependencyReport::default();
    }

    let mut state = ReverseResolverState::new(targets);

    for target in targets {
        let root = target.name.trim();
        if root.is_empty() {
            continue;
        }

        if state.pkg_info(root).is_none() {
            tracing::warn!(
                "Skipping reverse dependency walk for {} (not installed)",
                root
            );
            continue;
        }

        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(root.to_string());

        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        queue.push_back((root.to_string(), 0));

        while let Some((current, depth)) = queue.pop_front() {
            let Some(info) = state.pkg_info(&current) else {
                continue;
            };

            for dependent in info.required_by.iter().filter(|name| !name.is_empty()) {
                state.update_entry(dependent, &current, root, depth + 1);

                if visited.insert(dependent.to_string()) {
                    queue.push_back((dependent.to_string(), depth + 1));
                }
            }
        }
    }

    let ReverseResolverState { aggregated, .. } = state;

    let mut summary_map: HashMap<String, ReverseRootSummary> = HashMap::new();
    for entry in aggregated.values() {
        for (root, relation) in &entry.per_root {
            let summary = summary_map
                .entry(root.clone())
                .or_insert_with(|| ReverseRootSummary {
                    package: root.clone(),
                    ..Default::default()
                });

            if relation.parents.contains(root) || relation.min_depth() == 1 {
                summary.direct_dependents += 1;
            } else {
                summary.transitive_dependents += 1;
            }
            summary.total_dependents = summary.direct_dependents + summary.transitive_dependents;
        }
    }

    for target in targets {
        summary_map
            .entry(target.name.clone())
            .or_insert_with(|| ReverseRootSummary {
                package: target.name.clone(),
                ..Default::default()
            });
    }

    let mut summaries: Vec<ReverseRootSummary> = summary_map.into_values().collect();
    summaries.sort_by(|a, b| a.package.cmp(&b.package));

    let mut dependencies: Vec<DependencyInfo> = aggregated
        .into_iter()
        .map(|(name, entry)| convert_entry(name, entry))
        .collect();
    dependencies.sort_by(|a, b| a.name.cmp(&b.name));

    tracing::info!(
        "Reverse dependency resolution complete ({} impacted packages)",
        dependencies.len()
    );

    ReverseDependencyReport {
        dependencies,
        summaries,
    }
}

fn convert_entry(name: String, entry: AggregatedEntry) -> DependencyInfo {
    let AggregatedEntry {
        info,
        per_root,
        selected_for_removal,
    } = entry;

    let PkgInfo {
        name: pkg_name,
        version,
        repo,
        groups,
        required_by: _,
        explicit,
    } = info;

    let mut required_by: Vec<String> = per_root.keys().cloned().collect();
    required_by.sort();

    let mut all_parents: HashSet<String> = HashSet::new();
    for relation in per_root.values() {
        all_parents.extend(relation.parents.iter().cloned());
    }
    let mut depends_on: Vec<String> = all_parents.into_iter().collect();
    depends_on.sort();

    let mut reason_parts: Vec<String> = Vec::new();
    for (root, relation) in &per_root {
        let depth = relation.min_depth();
        let mut parents: Vec<String> = relation.parents.iter().cloned().collect();
        parents.sort();

        if depth <= 1 {
            reason_parts.push(format!("requires {}", root));
        } else {
            let via = if parents.is_empty() {
                "unknown".to_string()
            } else {
                parents.join(", ")
            };
            reason_parts.push(format!("blocks {} (depth {} via {})", root, depth, via));
        }
    }

    if selected_for_removal {
        reason_parts.push("already selected for removal".to_string());
    }
    if explicit {
        reason_parts.push("explicitly installed".to_string());
    }

    reason_parts.sort();
    let reason = if reason_parts.is_empty() {
        "required by removal targets".to_string()
    } else {
        reason_parts.join("; ")
    };

    let source = match repo.as_deref() {
        Some(repo) if repo.eq_ignore_ascii_case("local") || repo.is_empty() => {
            DependencySource::Local
        }
        Some(repo) => DependencySource::Official {
            repo: repo.to_string(),
        },
        None => DependencySource::Local,
    };

    let is_core = repo
        .as_deref()
        .map(|r| r.eq_ignore_ascii_case("core"))
        .unwrap_or(false);
    let is_system = groups
        .iter()
        .any(|g| matches!(g.as_str(), "base" | "base-devel"));

    let display_name = if pkg_name.is_empty() { name } else { pkg_name };

    DependencyInfo {
        name: display_name,
        version,
        status: DependencyStatus::Conflict { reason },
        source,
        required_by,
        depends_on,
        is_core,
        is_system,
    }
}

fn fetch_pkg_info(name: &str) -> Result<PkgInfo, String> {
    tracing::debug!("Running: pacman -Qi {}", name);
    let output = Command::new("pacman")
        .args(["-Qi", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|e| format!("pacman -Qi {} failed: {}", name, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "pacman -Qi {} exited with {:?}: {}",
            name, output.status, stderr
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let map = parse_key_value_output(&text);

    let required_by = split_ws_or_none(map.get("Required By"));
    let groups = split_ws_or_none(map.get("Groups"));
    let version = map.get("Version").cloned().unwrap_or_default();
    let repo = map.get("Repository").cloned();
    let install_reason = map
        .get("Install Reason")
        .cloned()
        .unwrap_or_default()
        .to_lowercase();
    let explicit = install_reason.contains("explicit");

    Ok(PkgInfo {
        name: map.get("Name").cloned().unwrap_or_else(|| name.to_string()),
        version,
        repo,
        groups,
        required_by,
        explicit,
    })
}

fn parse_key_value_output(text: &str) -> BTreeMap<String, String> {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    let mut last_key: Option<String> = None;

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            last_key = Some(key.clone());
            map.insert(key, val);
        } else if (line.starts_with(' ') || line.starts_with('\t'))
            && let Some(key) = &last_key
        {
            let entry = map.entry(key.clone()).or_default();
            if !entry.ends_with(' ') {
                entry.push(' ');
            }
            entry.push_str(line.trim());
        }
    }

    map
}

fn split_ws_or_none(field: Option<&String>) -> Vec<String> {
    match field {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
                Vec::new()
            } else {
                trimmed.split_whitespace().map(|s| s.to_string()).collect()
            }
        }
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::types::{PackageItem, Source};
    use std::collections::HashMap;

    fn pkg_item(name: &str) -> PackageItem {
        PackageItem {
            name: name.into(),
            version: "1.0".into(),
            description: "test".into(),
            source: Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        }
    }

    fn pkg_info_stub(name: &str) -> PkgInfo {
        PkgInfo {
            name: name.into(),
            version: "2.0".into(),
            repo: Some("extra".into()),
            groups: Vec::new(),
            required_by: Vec::new(),
            explicit: false,
        }
    }

    #[test]
    fn update_entry_tracks_root_relations_and_selection() {
        let targets = vec![pkg_item("root"), pkg_item("app")];
        let mut state = ReverseResolverState::new(&targets);
        state.cache.insert("app".into(), pkg_info_stub("app"));

        state.update_entry("app", "root", "root", 1);

        let entry = state
            .aggregated
            .get("app")
            .expect("aggregated entry populated");
        assert!(entry.selected_for_removal, "target membership flagged");
        assert_eq!(entry.info.name, "app");
        let relation = entry
            .per_root
            .get("root")
            .expect("relation stored for root");
        assert_eq!(relation.min_depth(), 1);
        assert!(relation.parents.contains("root"));
    }

    #[test]
    fn convert_entry_produces_conflict_reason_and_flags() {
        let mut relation_a = RootRelation::new();
        relation_a.record("root", 1);
        let mut relation_b = RootRelation::new();
        relation_b.record("parent_x", 2);
        relation_b.record("parent_y", 2);

        let entry = AggregatedEntry {
            info: PkgInfo {
                name: "dep_alias".into(),
                version: "3.1".into(),
                repo: Some("core".into()),
                groups: vec!["base".into()],
                required_by: Vec::new(),
                explicit: true,
            },
            per_root: HashMap::from([("root".into(), relation_a), ("other".into(), relation_b)]),
            selected_for_removal: true,
        };

        let info = convert_entry("dep".into(), entry);
        let DependencyStatus::Conflict { reason } = &info.status else {
            panic!("expected conflict status");
        };
        assert!(reason.contains("requires root"));
        assert!(reason.contains("blocks other"));
        assert!(reason.contains("already selected for removal"));
        assert!(reason.contains("explicitly installed"));
        assert_eq!(info.required_by, vec!["other", "root"]);
        assert_eq!(info.depends_on, vec!["parent_x", "parent_y", "root"]);
        assert!(info.is_core);
        assert!(info.is_system);
        assert_eq!(info.name, "dep_alias");
    }

    #[test]
    fn parse_key_value_output_merges_wrapped_lines() {
        let sample = "Name            : pkg\nDescription     : Short desc\n                continuation line\nRequired By     : foo bar\nInstall Reason  : Explicitly installed\n";
        let map = parse_key_value_output(sample);
        assert_eq!(map.get("Name"), Some(&"pkg".to_string()));
        assert_eq!(
            map.get("Description"),
            Some(&"Short desc continuation line".to_string())
        );
        assert_eq!(map.get("Required By"), Some(&"foo bar".to_string()));
    }

    #[test]
    fn split_ws_or_none_handles_none_and_empty() {
        assert!(split_ws_or_none(Some(&"None".to_string())).is_empty());
        assert!(split_ws_or_none(Some(&"   ".to_string())).is_empty());
        let list = split_ws_or_none(Some(&"foo bar".to_string()));
        assert_eq!(list, vec!["foo", "bar"]);
        assert!(split_ws_or_none(None).is_empty());
    }
}
