//! Deprecation marking, blocker analysis, and tree pruning.

use crate::{DeprecationSource, Packument, PackumentVersion, SourceError};
use riri_common::{LockGraph, LockGraphEdge, RootDepKind};
use riri_semver_range::ParsedRange;
use semver::Version;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

/// A package whose declared range blocks the fix for a deprecated dependency.
#[derive(Debug, Clone, Serialize)]
pub struct Blocker {
    pub name: String,
    pub version: String,
    /// Range the blocker declares for the deprecated package.
    pub requires: String,
    /// Newest non-deprecated version the fix needs.
    pub fix_needs: String,
}

/// One deprecated package instance and how (if at all) it can be fixed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeprecatedFinding {
    pub name: String,
    pub version: String,
    pub message: Option<String>,
    pub latest: Option<String>,
    pub update_fixable: bool,
    /// In-range newest non-deprecated version when update-fixable.
    pub fix_version: Option<String>,
    pub needs_replacement: bool,
    pub blockers: Vec<Blocker>,
    /// Direct dependencies (roots) whose subtree reaches this package.
    pub direct_dependents: Vec<String>,
}

/// A node in the rendered (and JSON) dependency tree.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderNode {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecated: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub circular: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deduped: bool,
    pub children: Vec<RenderNode>,
}

/// Full analysis result: pruned tree (when any deprecation) + flat findings.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree: Option<RenderNode>,
    pub deprecated: Vec<DeprecatedFinding>,
}

/// A resolved parent edge into a deprecated node, with the declared range.
struct ParentEdge {
    /// `None` when the parent is the project root.
    parent_node: Option<usize>,
    name: String,
    version: String,
    range: String,
}

/// Analyze a dependency graph against a deprecation source.
///
/// # Errors
/// Returns the first [`SourceError`] raised while fetching packuments.
pub fn analyze(
    graph: &LockGraph,
    project_name: &str,
    source: &dyn DeprecationSource,
) -> Result<Report, SourceError> {
    // 1. Fetch packuments for every unique package name.
    let mut names: Vec<String> = graph.nodes.iter().map(|n| n.name.clone()).collect();
    names.sort();
    names.dedup();
    let (packuments, errors) = crate::registry::fetch_all(source, &names);
    if let Some(e) = errors.into_iter().next() {
        return Err(e);
    }

    // 2. Mark deprecated nodes.
    let deprecated: Vec<bool> = (0..graph.nodes.len())
        .map(|i| {
            let n = &graph.nodes[i];
            packuments
                .get(&n.name)
                .and_then(|p| p.versions.get(&n.version))
                .is_some_and(PackumentVersion::is_deprecated)
        })
        .collect();

    // 3. Reverse adjacency + the "reaches a deprecated node" closure.
    let mut parents: Vec<Vec<usize>> = vec![Vec::new(); graph.nodes.len()];
    for (i, node) in graph.nodes.iter().enumerate() {
        for edge in &node.deps {
            parents[edge.node].push(i);
        }
    }
    let mut reaches: HashSet<usize> = HashSet::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (i, &dep) in deprecated.iter().enumerate() {
        if dep {
            reaches.insert(i);
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        for &p in &parents[i] {
            if reaches.insert(p) {
                queue.push_back(p);
            }
        }
    }

    let root_nodes: HashSet<usize> = graph.roots.iter().map(|r| r.node).collect();

    // Forward reachability from the declared roots. Only deprecations actually
    // pulled in by a root are reported — workspace members share one lockfile
    // graph, so a member must not be charged for another member's deps.
    let mut from_roots: HashSet<usize> = HashSet::new();
    let mut forward: VecDeque<usize> = VecDeque::new();
    for r in &graph.roots {
        if from_roots.insert(r.node) {
            forward.push_back(r.node);
        }
    }
    while let Some(i) = forward.pop_front() {
        for edge in &graph.nodes[i].deps {
            if from_roots.insert(edge.node) {
                forward.push_back(edge.node);
            }
        }
    }

    // 4. Per deprecated node: blocker analysis + fix labels.
    let mut dep_indices: Vec<usize> = (0..graph.nodes.len())
        .filter(|&i| deprecated[i] && from_roots.contains(&i))
        .collect();
    dep_indices.sort_by(|&a, &b| {
        (
            graph.nodes[a].name.as_str(),
            graph.nodes[a].version.as_str(),
        )
            .cmp(&(
                graph.nodes[b].name.as_str(),
                graph.nodes[b].version.as_str(),
            ))
    });

    let mut findings: Vec<DeprecatedFinding> = Vec::new();
    let mut node_fix: HashMap<usize, Vec<String>> = HashMap::new();

    for &d in &dep_indices {
        let (finding, labels) =
            analyze_node(graph, &parents, &packuments, &root_nodes, project_name, d);
        for (target, label) in labels {
            node_fix.entry(target).or_default().push(label);
        }
        findings.push(finding);
    }

    // 5. Pruned tree (only when something is deprecated).
    let tree = if dep_indices.is_empty() {
        None
    } else {
        Some(build_tree(
            graph,
            &packuments,
            &deprecated,
            &reaches,
            &node_fix,
            project_name,
        ))
    };

    Ok(Report {
        tree,
        deprecated: findings,
    })
}

/// Classify one deprecated node `d`: build its finding and the fix labels to
/// attach (keyed by target node index — blocker labels land on the parent,
/// update/replacement labels on the node itself).
fn analyze_node(
    graph: &LockGraph,
    parents: &[Vec<usize>],
    packuments: &HashMap<String, Packument>,
    root_nodes: &HashSet<usize>,
    project_name: &str,
    d: usize,
) -> (DeprecatedFinding, Vec<(usize, String)>) {
    let node = &graph.nodes[d];
    let pkg = packuments.get(&node.name);
    let latest = pkg.and_then(Packument::latest).map(str::to_string);
    let candidates = pkg.map(candidate_versions).unwrap_or_default();
    let newest = candidates.first().cloned();
    let message = pkg
        .and_then(|p| p.versions.get(&node.version))
        .and_then(PackumentVersion::deprecation_message)
        .map(str::to_string);

    let edges = parent_edges(graph, parents, packuments, project_name, d);

    let mut labels: Vec<(usize, String)> = Vec::new();
    let mut blockers: Vec<Blocker> = Vec::new();
    let mut in_range_fixes: Vec<Version> = Vec::new();
    let mut all_in_range = !edges.is_empty();
    for e in &edges {
        let fix = ParsedRange::parse(&e.range)
            .ok()
            .and_then(|pr| candidates.iter().find(|v| pr.satisfies(v)).cloned());
        if let Some(v) = fix {
            in_range_fixes.push(v);
        } else {
            all_in_range = false;
            if let (Some(p), Some(n)) = (e.parent_node, newest.as_ref()) {
                labels.push((
                    p,
                    format!(
                        "blocks: requires {}@{}, fix needs {} → {} update required",
                        node.name, e.range, n, e.name
                    ),
                ));
                blockers.push(Blocker {
                    name: e.name.clone(),
                    version: e.version.clone(),
                    requires: e.range.clone(),
                    fix_needs: n.to_string(),
                });
            }
        }
    }

    let needs_replacement = candidates.is_empty();
    let update_fixable = all_in_range && !candidates.is_empty();
    let fix_version = update_fixable
        .then(|| in_range_fixes.iter().min().cloned())
        .flatten();

    if let (true, Some(fv), Some(e)) = (update_fixable, fix_version.as_ref(), edges.first()) {
        labels.push((
            d,
            format!("fix: update {} — {} allows {}", node.name, e.range, fv),
        ));
    } else if needs_replacement {
        labels.push((
            d,
            "no non-deprecated version — needs replacement".to_string(),
        ));
    }

    let finding = DeprecatedFinding {
        name: node.name.clone(),
        version: node.version.clone(),
        message,
        latest,
        update_fixable,
        fix_version: fix_version.map(|v| v.to_string()),
        needs_replacement,
        blockers,
        direct_dependents: direct_dependents(graph, parents, root_nodes, d),
    };
    (finding, labels)
}

/// Non-deprecated, non-prerelease versions of a packument, newest first.
fn candidate_versions(pkg: &Packument) -> Vec<Version> {
    let mut versions: Vec<Version> = pkg
        .versions
        .iter()
        .filter(|(_, v)| !v.is_deprecated())
        .filter_map(|(k, _)| Version::parse(k).ok())
        .filter(|v| v.pre.is_empty())
        .collect();
    versions.sort();
    versions.reverse();
    versions
}

/// Registry `latest` when it parses strictly newer than `version`.
fn newer_latest(pkg: Option<&Packument>, version: &str) -> Option<String> {
    let pkg = pkg?;
    let latest = pkg.latest()?;
    let current = Version::parse(version).ok()?;
    let candidate = Version::parse(latest).ok()?;
    (candidate > current).then(|| latest.to_string())
}

/// All parent edges into deprecated node `d` with their declared ranges:
/// intermediate package parents (range from the parent's packument entry) plus
/// project-root parents (range from `package.json`).
fn parent_edges(
    graph: &LockGraph,
    parents: &[Vec<usize>],
    packuments: &HashMap<String, Packument>,
    project_name: &str,
    d: usize,
) -> Vec<ParentEdge> {
    let name = &graph.nodes[d].name;
    let mut edges: Vec<ParentEdge> = Vec::new();
    for &p in &parents[d] {
        let parent = &graph.nodes[p];
        if let Some(range) = packuments
            .get(&parent.name)
            .and_then(|pk| pk.versions.get(&parent.version))
            .and_then(|v| v.declared_range(name))
        {
            edges.push(ParentEdge {
                parent_node: Some(p),
                name: parent.name.clone(),
                version: parent.version.clone(),
                range: range.to_string(),
            });
        }
    }
    for r in graph.roots.iter().filter(|r| r.node == d) {
        edges.push(ParentEdge {
            parent_node: None,
            name: project_name.to_string(),
            version: String::new(),
            range: r.range.clone(),
        });
    }
    edges
}

/// Names of direct dependencies (roots) whose subtree reaches `d`.
fn direct_dependents(
    graph: &LockGraph,
    parents: &[Vec<usize>],
    root_nodes: &HashSet<usize>,
    d: usize,
) -> Vec<String> {
    let mut seen: HashSet<usize> = HashSet::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    seen.insert(d);
    queue.push_back(d);
    let mut names: Vec<String> = Vec::new();
    while let Some(i) = queue.pop_front() {
        if root_nodes.contains(&i) {
            names.push(graph.nodes[i].name.clone());
        }
        for &p in &parents[i] {
            if seen.insert(p) {
                queue.push_back(p);
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Build the synthetic-root tree over the pruned graph.
fn build_tree(
    graph: &LockGraph,
    packuments: &HashMap<String, Packument>,
    deprecated: &[bool],
    reaches: &HashSet<usize>,
    node_fix: &HashMap<usize, Vec<String>>,
    project_name: &str,
) -> RenderNode {
    let mut roots: Vec<_> = graph
        .roots
        .iter()
        .filter(|r| reaches.contains(&r.node))
        .collect();
    roots.sort_by(|a, b| node_order(graph, a.node).cmp(&node_order(graph, b.node)));

    let mut path: Vec<usize> = Vec::new();
    let mut printed: HashSet<usize> = HashSet::new();
    let children = roots
        .into_iter()
        .map(|r| {
            build_node(
                graph,
                packuments,
                deprecated,
                reaches,
                node_fix,
                r.node,
                true,
                Some(r.kind),
                &mut path,
                &mut printed,
            )
        })
        .collect();

    RenderNode {
        name: project_name.to_string(),
        version: None,
        kind: None,
        latest: None,
        deprecated: None,
        fix: None,
        circular: false,
        deduped: false,
        children,
    }
}

fn node_order(graph: &LockGraph, i: usize) -> (&str, &str) {
    (
        graph.nodes[i].name.as_str(),
        graph.nodes[i].version.as_str(),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_node(
    graph: &LockGraph,
    packuments: &HashMap<String, Packument>,
    deprecated: &[bool],
    reaches: &HashSet<usize>,
    node_fix: &HashMap<usize, Vec<String>>,
    i: usize,
    is_root_level: bool,
    root_kind: Option<RootDepKind>,
    path: &mut Vec<usize>,
    printed: &mut HashSet<usize>,
) -> RenderNode {
    let node = &graph.nodes[i];
    let kind = root_kind.and_then(|k| match k {
        RootDepKind::DevDependencies => Some("dev"),
        RootDepKind::OptionalDependencies => Some("optional"),
        RootDepKind::Dependencies => None,
    });
    let is_deprecated = deprecated[i];
    let deprecated_msg = is_deprecated.then(|| {
        packuments
            .get(&node.name)
            .and_then(|p| p.versions.get(&node.version))
            .and_then(PackumentVersion::deprecation_message)
            .map(|m| m.lines().next().unwrap_or("").to_string())
            .unwrap_or_default()
    });
    let fix = node_fix.get(&i).map(|labels| labels.join("; "));

    let base = |latest, circular, deduped, children| RenderNode {
        name: node.name.clone(),
        version: Some(node.version.clone()),
        kind,
        latest,
        deprecated: deprecated_msg.clone(),
        fix: fix.clone(),
        circular,
        deduped,
        children,
    };

    if path.contains(&i) {
        return base(None, true, false, Vec::new());
    }
    if printed.contains(&i) {
        return base(None, false, true, Vec::new());
    }
    printed.insert(i);
    path.push(i);

    let latest = if is_root_level || is_deprecated {
        newer_latest(packuments.get(&node.name), &node.version)
    } else {
        None
    };

    let mut child_edges: Vec<&LockGraphEdge> = node
        .deps
        .iter()
        .filter(|e| reaches.contains(&e.node))
        .collect();
    child_edges.sort_by(|a, b| node_order(graph, a.node).cmp(&node_order(graph, b.node)));
    let children = child_edges
        .into_iter()
        .map(|e| {
            build_node(
                graph, packuments, deprecated, reaches, node_fix, e.node, false, None, path,
                printed,
            )
        })
        .collect();

    path.pop();
    base(latest, false, false, children)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use riri_common::{LockGraphNode, LockGraphRoot};

    struct StubSource(HashMap<String, Packument>);
    impl DeprecationSource for StubSource {
        fn packument(&self, name: &str) -> Result<Option<Packument>, SourceError> {
            Ok(self.0.get(name).cloned())
        }
    }
    fn stub(map: &[(&str, &str)]) -> StubSource {
        StubSource(
            map.iter()
                .map(|(n, j)| {
                    (
                        (*n).to_string(),
                        serde_json::from_str::<Packument>(j).unwrap(),
                    )
                })
                .collect(),
        )
    }

    fn node(name: &str, version: &str, deps: &[usize]) -> LockGraphNode {
        LockGraphNode {
            name: name.into(),
            version: version.into(),
            deps: deps
                .iter()
                .map(|&n| LockGraphEdge {
                    node: n,
                    optional: false,
                })
                .collect(),
        }
    }
    fn root(range: &str, node: usize) -> LockGraphRoot {
        LockGraphRoot {
            kind: RootDepKind::Dependencies,
            range: range.into(),
            node,
        }
    }

    /// Walk the tree collecting every node matching a predicate.
    fn collect<'a>(
        n: &'a RenderNode,
        pred: &dyn Fn(&RenderNode) -> bool,
        out: &mut Vec<&'a RenderNode>,
    ) {
        if pred(n) {
            out.push(n);
        }
        for c in &n.children {
            collect(c, pred, out);
        }
    }

    #[test]
    fn transitive_deprecated_prunes_non_deprecated_siblings() {
        let graph = LockGraph {
            nodes: vec![
                node("a", "1.0.0", &[1, 2]),
                node("b", "1.0.0", &[]),
                node("c", "1.0.0", &[]),
            ],
            roots: vec![root("^1.0.0", 0)],
        };
        let source = stub(&[
            (
                "a",
                r#"{"versions": {"1.0.0": {"dependencies": {"b": "^1.0.0", "c": "^1.0.0"}}}}"#,
            ),
            ("b", r#"{"versions": {"1.0.0": {"deprecated": "gone"}}}"#),
            ("c", r#"{"versions": {"1.0.0": {}}}"#),
        ]);
        let report = analyze(&graph, "proj", &source).unwrap();
        assert_eq!(report.deprecated.len(), 1);
        let tree = report.tree.unwrap();
        let a = &tree.children[0];
        assert_eq!(a.name, "a");
        // only b survives the prune; c (non-deprecated leaf) is dropped.
        assert_eq!(a.children.len(), 1);
        assert_eq!(a.children[0].name, "b");
    }

    #[test]
    fn parent_range_admits_newer_is_update_fixable() {
        let graph = LockGraph {
            nodes: vec![node("bar", "2.0.0", &[1]), node("foo", "1.0.0", &[])],
            roots: vec![root("^2.0.0", 0)],
        };
        let source = stub(&[
            (
                "bar",
                r#"{"dist-tags": {"latest": "2.0.0"}, "versions": {"2.0.0": {"dependencies": {"foo": "^1.0.0"}}}}"#,
            ),
            (
                "foo",
                r#"{"dist-tags": {"latest": "1.4.2"}, "versions": {"1.0.0": {"deprecated": "old"}, "1.4.2": {}}}"#,
            ),
        ]);
        let report = analyze(&graph, "proj", &source).unwrap();
        let f = &report.deprecated[0];
        assert!(f.update_fixable);
        assert_eq!(f.fix_version.as_deref(), Some("1.4.2"));
        assert!(f.blockers.is_empty());
    }

    #[test]
    fn parent_range_excluding_target_is_blocker() {
        let graph = LockGraph {
            nodes: vec![node("bar", "2.3.1", &[1]), node("foo", "1.0.0", &[])],
            roots: vec![root("^2.0.0", 0)],
        };
        let source = stub(&[
            (
                "bar",
                r#"{"dist-tags": {"latest": "2.3.1"}, "versions": {"2.3.1": {"dependencies": {"foo": "~1.0.0"}}}}"#,
            ),
            (
                "foo",
                r#"{"dist-tags": {"latest": "2.1.0"}, "versions": {"1.0.0": {"deprecated": "use @foo/core instead"}, "2.1.0": {}}}"#,
            ),
        ]);
        let report = analyze(&graph, "my-project", &source).unwrap();
        let f = &report.deprecated[0];
        assert_eq!((f.name.as_str(), f.version.as_str()), ("foo", "1.0.0"));
        assert!(!f.update_fixable);
        assert_eq!(f.blockers[0].name, "bar");
        assert_eq!(f.blockers[0].requires, "~1.0.0");
        assert_eq!(f.blockers[0].fix_needs, "2.1.0");
        assert_eq!(f.direct_dependents, vec!["bar".to_string()]);
    }

    #[test]
    fn all_versions_deprecated_needs_replacement() {
        let graph = LockGraph {
            nodes: vec![node("foo", "1.0.0", &[])],
            roots: vec![root("^1.0.0", 0)],
        };
        let source = stub(&[(
            "foo",
            r#"{"versions": {"1.0.0": {"deprecated": "dead"}, "1.1.0": {"deprecated": "dead"}}}"#,
        )]);
        let report = analyze(&graph, "proj", &source).unwrap();
        let f = &report.deprecated[0];
        assert!(f.needs_replacement);
        assert!(!f.update_fixable);
    }

    #[test]
    fn deprecated_direct_dep_uses_root_range() {
        let graph = LockGraph {
            nodes: vec![node("foo", "1.0.0", &[])],
            roots: vec![root("^1.0.0", 0)],
        };
        let source = stub(&[(
            "foo",
            r#"{"dist-tags": {"latest": "1.5.0"}, "versions": {"1.0.0": {"deprecated": "old"}, "1.5.0": {}}}"#,
        )]);
        let report = analyze(&graph, "proj", &source).unwrap();
        let f = &report.deprecated[0];
        assert!(f.update_fixable);
        assert_eq!(f.fix_version.as_deref(), Some("1.5.0"));
        assert_eq!(f.direct_dependents, vec!["foo".to_string()]);
    }

    #[test]
    fn cycle_terminates_and_marks_circular() {
        let graph = LockGraph {
            nodes: vec![node("a", "1.0.0", &[1]), node("b", "1.0.0", &[0])],
            roots: vec![root("^1.0.0", 0)],
        };
        let source = stub(&[
            (
                "a",
                r#"{"versions": {"1.0.0": {"dependencies": {"b": "^1.0.0"}}}}"#,
            ),
            (
                "b",
                r#"{"versions": {"1.0.0": {"deprecated": "old", "dependencies": {"a": "^1.0.0"}}}}"#,
            ),
        ]);
        let report = analyze(&graph, "proj", &source).unwrap();
        let tree = report.tree.unwrap();
        let mut circular = Vec::new();
        collect(&tree, &|n| n.circular, &mut circular);
        assert!(!circular.is_empty());
    }

    #[test]
    fn second_occurrence_of_deprecated_node_is_deduped() {
        // root → x → dep, root → y → dep (dep is one shared node, index 2).
        let graph = LockGraph {
            nodes: vec![
                node("x", "1.0.0", &[2]),
                node("y", "1.0.0", &[2]),
                node("dep", "1.0.0", &[]),
            ],
            roots: vec![root("^1.0.0", 0), root("^1.0.0", 1)],
        };
        let source = stub(&[
            (
                "x",
                r#"{"versions": {"1.0.0": {"dependencies": {"dep": "^1.0.0"}}}}"#,
            ),
            (
                "y",
                r#"{"versions": {"1.0.0": {"dependencies": {"dep": "^1.0.0"}}}}"#,
            ),
            ("dep", r#"{"versions": {"1.0.0": {"deprecated": "gone"}}}"#),
        ]);
        let report = analyze(&graph, "proj", &source).unwrap();
        let tree = report.tree.unwrap();
        let mut deps = Vec::new();
        collect(&tree, &|n| n.name == "dep", &mut deps);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps.iter().filter(|n| n.deduped).count(), 1);
    }

    #[test]
    fn nothing_deprecated_yields_no_tree() {
        let graph = LockGraph {
            nodes: vec![node("a", "1.0.0", &[])],
            roots: vec![root("^1.0.0", 0)],
        };
        let source = stub(&[("a", r#"{"versions": {"1.0.0": {}}}"#)]);
        let report = analyze(&graph, "proj", &source).unwrap();
        assert!(report.deprecated.is_empty());
        assert!(report.tree.is_none());
    }
}
