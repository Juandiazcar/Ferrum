//! `cargo xtask check-deps`: validates the crate graph against `xtask/deps-rules.toml`
//! (the machine-readable form of docs/04 §2).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use cargo_metadata::{DependencyKind, MetadataCommand};
use serde::Deserialize;

use crate::out;

const RULES_PATH: &str = "xtask/deps-rules.toml";
const TESTKIT: &str = "ferrum-testkit";

#[derive(Deserialize)]
pub(crate) struct Rules {
    allowed: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    forbidden_external: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `[dependencies]` and `[build-dependencies]`.
    Normal,
    /// `[dev-dependencies]`.
    Dev,
}

pub(crate) struct Edge {
    pub(crate) to: String,
    pub(crate) kind: Kind,
}

pub(crate) struct Node {
    pub(crate) name: String,
    /// Edges to other workspace crates.
    pub(crate) internal: Vec<Edge>,
    /// Names of direct third-party normal dependencies.
    pub(crate) external: Vec<String>,
}

/// Returns human-readable violations; empty means the graph is valid.
pub(crate) fn validate(nodes: &[Node], rules: &Rules) -> Vec<String> {
    let mut errors = Vec::new();
    let names: BTreeSet<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
    for name in rules.allowed.keys().filter(|name| !names.contains(name.as_str())) {
        errors.push(format!("rules list `{name}` but it is not a workspace crate"));
    }
    for node in nodes {
        check_node(node, rules, &mut errors);
    }
    errors.extend(find_cycles(nodes));
    errors
}

fn check_node(node: &Node, rules: &Rules, errors: &mut Vec<String>) {
    let Some(allowed) = rules.allowed.get(&node.name) else {
        errors.push(format!("`{}` has no entry in {RULES_PATH}", node.name));
        return;
    };
    for edge in &node.internal {
        let listed = allowed.contains(&edge.to);
        let ok = match edge.kind {
            Kind::Normal => listed,
            Kind::Dev => listed || (edge.to == TESTKIT && node.name != TESTKIT),
        };
        if !ok {
            let section = if edge.kind == Kind::Dev { "dev-dependency" } else { "dependency" };
            errors.push(format!("forbidden {section} edge: {} -> {}", node.name, edge.to));
        }
    }
    let banned = rules.forbidden_external.get(&node.name).map_or(&[][..], Vec::as_slice);
    for external in node.external.iter().filter(|dep| banned.contains(dep)) {
        errors.push(format!("`{}` must never depend on `{external}`", node.name));
    }
}

/// Cycles over normal edges only: dev-dependencies on the testkit legitimately point back.
fn find_cycles(nodes: &[Node]) -> Vec<String> {
    let graph: BTreeMap<&str, Vec<&str>> = nodes
        .iter()
        .map(|node| {
            let targets = node
                .internal
                .iter()
                .filter(|edge| edge.kind == Kind::Normal)
                .map(|edge| edge.to.as_str())
                .collect();
            (node.name.as_str(), targets)
        })
        .collect();
    let mut walk = Walk { graph, done: BTreeSet::new(), errors: Vec::new() };
    for start in walk.graph.keys().copied().collect::<Vec<_>>() {
        walk.visit(start, &mut Vec::new());
    }
    walk.errors
}

/// Depth-first traversal state for cycle detection.
struct Walk<'a> {
    graph: BTreeMap<&'a str, Vec<&'a str>>,
    done: BTreeSet<&'a str>,
    errors: Vec<String>,
}

impl<'a> Walk<'a> {
    fn visit(&mut self, node: &'a str, path: &mut Vec<&'a str>) {
        if let Some(position) = path.iter().position(|seen| *seen == node) {
            let mut cycle = path.get(position..).unwrap_or_default().to_vec();
            cycle.push(node);
            self.errors.push(format!("dependency cycle: {}", cycle.join(" -> ")));
            return;
        }
        if !self.done.insert(node) {
            return;
        }
        path.push(node);
        for next in self.graph.get(node).cloned().unwrap_or_default() {
            self.visit(next, path);
        }
        path.pop();
    }
}

fn load_nodes() -> Result<Vec<Node>> {
    let metadata = MetadataCommand::new().exec().context("`cargo metadata` failed")?;
    let members: BTreeSet<_> = metadata.workspace_members.iter().collect();
    let workspace: Vec<_> = metadata.packages.iter().filter(|p| members.contains(&p.id)).collect();
    let names: BTreeSet<&str> = workspace.iter().map(|p| p.name.as_str()).collect();
    let nodes = workspace
        .iter()
        .map(|package| {
            let mut node =
                Node { name: package.name.to_string(), internal: Vec::new(), external: Vec::new() };
            for dep in &package.dependencies {
                let kind =
                    if dep.kind == DependencyKind::Development { Kind::Dev } else { Kind::Normal };
                if names.contains(dep.name.as_str()) {
                    node.internal.push(Edge { to: dep.name.clone(), kind });
                } else if kind == Kind::Normal {
                    node.external.push(dep.name.clone());
                }
            }
            node
        })
        .collect();
    Ok(nodes)
}

pub(crate) fn run() -> Result<bool> {
    let text = std::fs::read_to_string(Path::new(RULES_PATH))
        .with_context(|| format!("cannot read {RULES_PATH} (run from the repository root)"))?;
    let rules: Rules = toml::from_str(&text).with_context(|| format!("invalid {RULES_PATH}"))?;
    let nodes = load_nodes()?;
    let errors = validate(&nodes, &rules);
    for error in &errors {
        out::error(&format!("check-deps: {error}"));
    }
    if errors.is_empty() {
        out::info(&format!(
            "check-deps: {} workspace crates, layering rules satisfied",
            nodes.len()
        ));
    }
    Ok(errors.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(allowed: &[(&str, &[&str])], banned: &[(&str, &[&str])]) -> Rules {
        let owned = |items: &[&str]| items.iter().map(ToString::to_string).collect();
        Rules {
            allowed: allowed.iter().map(|(k, v)| ((*k).to_string(), owned(v))).collect(),
            forbidden_external: banned.iter().map(|(k, v)| ((*k).to_string(), owned(v))).collect(),
        }
    }

    fn node(name: &str, internal: &[(&str, Kind)], external: &[&str]) -> Node {
        Node {
            name: name.to_string(),
            internal: internal
                .iter()
                .map(|(to, kind)| Edge { to: (*to).to_string(), kind: *kind })
                .collect(),
            external: external.iter().map(ToString::to_string).collect(),
        }
    }

    fn base_rules() -> Rules {
        rules(&[("a", &["b"]), ("b", &[])], &[("a", &["winit"])])
    }

    #[test]
    fn accepts_listed_edges() {
        let nodes = [node("a", &[("b", Kind::Normal)], &[]), node("b", &[], &[])];
        assert_eq!(validate(&nodes, &base_rules()), Vec::<String>::new());
    }

    #[test]
    fn rejects_unlisted_edge() {
        let nodes = [node("a", &[], &[]), node("b", &[("a", Kind::Normal)], &[])];
        let errors = validate(&nodes, &base_rules());
        assert_eq!(errors, ["forbidden dependency edge: b -> a"]);
    }

    #[test]
    fn testkit_is_dev_only() {
        let allowed = [("a", &[][..]), ("ferrum-testkit", &[][..])];
        let dev =
            [node("a", &[("ferrum-testkit", Kind::Dev)], &[]), node("ferrum-testkit", &[], &[])];
        assert_eq!(validate(&dev, &rules(&allowed, &[])), Vec::<String>::new());
        let normal =
            [node("a", &[("ferrum-testkit", Kind::Normal)], &[]), node("ferrum-testkit", &[], &[])];
        assert_eq!(validate(&normal, &rules(&allowed, &[])).len(), 1);
    }

    #[test]
    fn detects_cycles() {
        let allowed = [("a", &["b"][..]), ("b", &["a"][..])];
        let nodes =
            [node("a", &[("b", Kind::Normal)], &[]), node("b", &[("a", Kind::Normal)], &[])];
        let errors = validate(&nodes, &rules(&allowed, &[]));
        assert!(errors.iter().any(|e| e.starts_with("dependency cycle: a -> b -> a")));
    }

    #[test]
    fn rejects_forbidden_external_and_missing_rules() {
        let nodes = [node("a", &[], &["winit"]), node("b", &[], &[]), node("c", &[], &[])];
        let with_ghost = rules(&[("a", &[]), ("b", &[]), ("ghost", &[])], &[("a", &["winit"])]);
        let errors = validate(&nodes, &with_ghost);
        assert!(errors.contains(&"`a` must never depend on `winit`".to_string()));
        assert!(errors.contains(&"`c` has no entry in xtask/deps-rules.toml".to_string()));
        assert!(errors.contains(&"rules list `ghost` but it is not a workspace crate".to_string()));
    }

    #[test]
    fn real_rules_file_parses() {
        let text = include_str!("../deps-rules.toml");
        let parsed: Rules = toml::from_str(text).expect("deps-rules.toml must parse");
        assert!(parsed.allowed.contains_key("ferrum-app"));
    }
}
