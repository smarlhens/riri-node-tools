//! `LockfileGraph` for npm lockfiles.

use crate::{NpmEntryDeps, NpmLockEntry, NpmPackageLock};
use riri_common::{
    GraphError, LockGraph, LockGraphEdge, LockGraphNode, LockGraphRoot, LockfileGraph, PackageJson,
    RootDepKind, is_local_specifier,
};
use std::collections::HashMap;

impl LockfileGraph for NpmPackageLock {
    fn dep_graph(&self, package_json: &PackageJson) -> Result<LockGraph, GraphError> {
        match self {
            Self::V2 {
                packages: Some(p), ..
            }
            | Self::V3 { packages: p } => Ok(graph_from_packages(p, package_json)),
            Self::V1 { dependencies } | Self::V2 { dependencies, .. } => {
                Ok(graph_from_nested(dependencies, package_json))
            }
        }
    }
}

const NODE_MODULES: &str = "node_modules/";

/// Package name from a v2/v3 `packages` key — portion after the last
/// `node_modules/`, or the whole key for workspace entries (no segment).
fn node_name_from_key(key: &str) -> &str {
    match key.rfind(NODE_MODULES) {
        Some(i) => &key[i + NODE_MODULES.len()..],
        None => key,
    }
}

/// Resolve dep `name` required from the package at `from_key` by walking
/// ancestor `node_modules` scopes, innermost first; follows `link:` entries.
fn resolve_key(
    packages: &HashMap<String, NpmLockEntry>,
    from_key: &str,
    name: &str,
) -> Option<String> {
    let mut scope = from_key.to_string();
    loop {
        let candidate = if scope.is_empty() {
            format!("node_modules/{name}")
        } else {
            format!("{scope}/node_modules/{name}")
        };
        if let Some(entry) = packages.get(&candidate) {
            if entry.link == Some(true) {
                if let Some(target) = entry
                    .resolved
                    .as_ref()
                    .filter(|r| packages.contains_key(*r))
                {
                    return Some(target.clone());
                }
                return None;
            }
            return Some(candidate);
        }
        if scope.is_empty() {
            return None;
        }
        scope = match scope.rfind("/node_modules/") {
            Some(i) => scope[..i].to_string(),
            None => String::new(),
        };
    }
}

/// Build a graph from the v2/v3 `packages` map (path-keyed, hoisted).
fn graph_from_packages(
    packages: &HashMap<String, NpmLockEntry>,
    package_json: &PackageJson,
) -> LockGraph {
    let mut graph = LockGraph::default();
    let mut key_to_idx: HashMap<String, usize> = HashMap::new();

    let mut keys: Vec<&String> = packages.keys().collect();
    keys.sort();

    // Phase 1: one node per non-root, non-link entry that has a version.
    for key in &keys {
        if key.is_empty() {
            continue;
        }
        let entry = &packages[*key];
        if entry.link == Some(true) {
            continue;
        }
        let Some(version) = entry.version.as_ref() else {
            continue;
        };
        let idx = graph.nodes.len();
        graph.nodes.push(LockGraphNode {
            name: node_name_from_key(key).to_string(),
            version: version.clone(),
            deps: Vec::new(),
        });
        key_to_idx.insert((*key).clone(), idx);
    }

    // Phase 2: edges from `dependencies` (Ranges) + `optionalDependencies`.
    for key in &keys {
        if key.is_empty() {
            continue;
        }
        let Some(&from_idx) = key_to_idx.get(*key) else {
            continue;
        };
        let entry = &packages[*key];
        let mut edges: Vec<(String, bool)> = Vec::new();
        if let Some(NpmEntryDeps::Ranges(deps)) = &entry.dependencies {
            edges.extend(deps.keys().map(|n| (n.clone(), false)));
        }
        if let Some(opt) = &entry.optional_dependencies {
            edges.extend(opt.keys().map(|n| (n.clone(), true)));
        }
        edges.sort();
        for (dep_name, optional) in edges {
            if let Some(target_key) = resolve_key(packages, key, &dep_name)
                && let Some(&target) = key_to_idx.get(&target_key)
            {
                graph.nodes[from_idx].deps.push(LockGraphEdge {
                    node: target,
                    optional,
                });
            }
        }
    }

    add_roots(&mut graph, package_json, |name| {
        resolve_key(packages, "", name).and_then(|k| key_to_idx.get(&k).copied())
    });
    graph
}

/// Build a graph from a v1 nested `dependencies` tree.
fn graph_from_nested(
    dependencies: &HashMap<String, NpmLockEntry>,
    package_json: &PackageJson,
) -> LockGraph {
    let mut graph = LockGraph::default();
    let mut stack: Vec<HashMap<String, usize>> = Vec::new();
    let top_scope = walk_nested(dependencies, &mut graph, &mut stack);
    add_roots(&mut graph, package_json, |name| {
        top_scope.get(name).copied()
    });
    graph
}

/// Recursively allocate nodes and resolve `requires` edges for one nesting
/// level. Returns the level's `name → node index` scope.
fn walk_nested(
    deps: &HashMap<String, NpmLockEntry>,
    graph: &mut LockGraph,
    stack: &mut Vec<HashMap<String, usize>>,
) -> HashMap<String, usize> {
    let mut keys: Vec<&String> = deps.keys().collect();
    keys.sort();

    let mut scope: HashMap<String, usize> = HashMap::new();
    for name in &keys {
        if let Some(version) = deps[*name].version.as_ref() {
            let idx = graph.nodes.len();
            graph.nodes.push(LockGraphNode {
                name: (*name).clone(),
                version: version.clone(),
                deps: Vec::new(),
            });
            scope.insert((*name).clone(), idx);
        }
    }

    stack.push(scope.clone());
    for name in &keys {
        let entry = &deps[*name];
        let Some(idx) = scope.get(*name).copied() else {
            continue;
        };
        let nested_scope = match &entry.dependencies {
            Some(NpmEntryDeps::Nested(nested)) => walk_nested(nested, graph, stack),
            _ => HashMap::new(),
        };
        stack.push(nested_scope);
        if let Some(requires) = &entry.requires {
            let mut req: Vec<&String> = requires.keys().collect();
            req.sort();
            for dep_name in req {
                if let Some(target) = resolve_in_stack(stack, dep_name) {
                    graph.nodes[idx].deps.push(LockGraphEdge {
                        node: target,
                        optional: false,
                    });
                }
            }
        }
        stack.pop();
    }
    stack.pop();
    scope
}

/// Resolve `name` against a scope stack, innermost (last) first.
fn resolve_in_stack(stack: &[HashMap<String, usize>], name: &str) -> Option<usize> {
    stack
        .iter()
        .rev()
        .find_map(|scope| scope.get(name).copied())
}

/// Push project-root edges for each `package.json` dependency bucket, skipping
/// local specifiers and names the resolver cannot place.
fn add_roots(
    graph: &mut LockGraph,
    package_json: &PackageJson,
    resolve: impl Fn(&str) -> Option<usize>,
) {
    let buckets = [
        (RootDepKind::Dependencies, &package_json.dependencies),
        (RootDepKind::DevDependencies, &package_json.dev_dependencies),
        (
            RootDepKind::OptionalDependencies,
            &package_json.optional_dependencies,
        ),
    ];
    for (kind, bucket) in buckets {
        let Some(bucket) = bucket else { continue };
        let mut entries: Vec<(&String, &String)> = bucket.iter().collect();
        entries.sort();
        for (name, range) in entries {
            if is_local_specifier(range) {
                continue;
            }
            if let Some(node) = resolve(name) {
                graph.roots.push(LockGraphRoot {
                    kind,
                    range: range.clone(),
                    node,
                });
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn pkg(json: &str) -> PackageJson {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn v3_root_dev_and_transitive_edges() {
        let lock = NpmPackageLock::parse(
            r#"{
            "lockfileVersion": 3,
            "packages": {
                "": {},
                "node_modules/a": {"version": "1.0.0", "dependencies": {"b": "^2.0.0"}},
                "node_modules/b": {"version": "2.5.0"},
                "node_modules/d": {"version": "9.0.0"}
            }
        }"#,
        )
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}, "devDependencies": {"d": "^9.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        assert_eq!(graph.roots.len(), 2);
        // sorted: Dependencies bucket first (a), then DevDependencies (d).
        assert_eq!(graph.roots[0].kind, RootDepKind::Dependencies);
        assert_eq!(graph.roots[1].kind, RootDepKind::DevDependencies);
        let a = &graph.nodes[graph.roots[0].node];
        assert_eq!((a.name.as_str(), a.version.as_str()), ("a", "1.0.0"));
        let b = &graph.nodes[a.deps[0].node];
        assert_eq!((b.name.as_str(), b.version.as_str()), ("b", "2.5.0"));
        let d = &graph.nodes[graph.roots[1].node];
        assert_eq!(d.name, "d");
    }

    #[test]
    fn v3_nested_override_beats_hoisted() {
        let lock = NpmPackageLock::parse(
            r#"{
            "lockfileVersion": 3,
            "packages": {
                "": {"dependencies": {"a": "^1.0.0"}},
                "node_modules/a": {"version": "1.0.0", "dependencies": {"b": "^2.0.0"}},
                "node_modules/b": {"version": "1.0.0"},
                "node_modules/a/node_modules/b": {"version": "2.5.0"}
            }
        }"#,
        )
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        assert_eq!(graph.roots.len(), 1);
        let a = &graph.nodes[graph.roots[0].node];
        assert_eq!((a.name.as_str(), a.version.as_str()), ("a", "1.0.0"));
        let b = &graph.nodes[a.deps[0].node];
        assert_eq!((b.name.as_str(), b.version.as_str()), ("b", "2.5.0"));
    }

    #[test]
    fn v3_optional_dependency_flagged() {
        let lock = NpmPackageLock::parse(
            r#"{
            "lockfileVersion": 3,
            "packages": {
                "": {"dependencies": {"a": "^1.0.0"}},
                "node_modules/a": {"version": "1.0.0", "optionalDependencies": {"b": "^2.0.0"}},
                "node_modules/b": {"version": "2.0.0"}
            }
        }"#,
        )
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        assert_eq!(a.deps.len(), 1);
        assert!(a.deps[0].optional);
        assert_eq!(graph.nodes[a.deps[0].node].name, "b");
    }

    #[test]
    fn v3_workspace_link_followed_through_resolved() {
        let lock = NpmPackageLock::parse(
            r#"{
            "lockfileVersion": 3,
            "packages": {
                "": {"dependencies": {"foo": "^1.0.0"}},
                "node_modules/foo": {"link": true, "resolved": "packages/foo"},
                "packages/foo": {"version": "1.0.0", "dependencies": {"bar": "^2.0.0"}},
                "node_modules/bar": {"version": "2.0.0"}
            }
        }"#,
        )
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"foo": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        assert_eq!(graph.roots.len(), 1);
        let foo = &graph.nodes[graph.roots[0].node];
        assert_eq!(foo.version, "1.0.0");
        let bar = &graph.nodes[foo.deps[0].node];
        assert_eq!((bar.name.as_str(), bar.version.as_str()), ("bar", "2.0.0"));
    }

    #[test]
    fn v3_missing_entry_skips_edge() {
        let lock = NpmPackageLock::parse(
            r#"{
            "lockfileVersion": 3,
            "packages": {
                "": {"dependencies": {"a": "^1.0.0"}},
                "node_modules/a": {"version": "1.0.0", "dependencies": {"ghost": "^9.0.0"}}
            }
        }"#,
        )
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        let a = &graph.nodes[graph.roots[0].node];
        assert!(a.deps.is_empty());
    }

    #[test]
    fn v3_local_specifiers_skipped_from_roots() {
        let lock = NpmPackageLock::parse(
            r#"{
            "lockfileVersion": 3,
            "packages": {
                "": {"dependencies": {"a": "^1.0.0"}},
                "node_modules/a": {"version": "1.0.0"}
            }
        }"#,
        )
        .unwrap();
        let pkg = pkg(
            r#"{"dependencies": {"a": "^1.0.0", "local": "file:../local", "ws": "workspace:*"}}"#,
        );
        let graph = lock.dep_graph(&pkg).unwrap();
        assert_eq!(graph.roots.len(), 1);
        assert_eq!(graph.nodes[graph.roots[0].node].name, "a");
    }

    #[test]
    fn v1_nested_requires_resolved_nearest_ancestor() {
        let lock = NpmPackageLock::parse(
            r#"{
            "lockfileVersion": 1,
            "dependencies": {
                "a": {
                    "version": "1.0.0",
                    "requires": {"b": "^2.0.0"},
                    "dependencies": {"b": {"version": "2.5.0"}}
                },
                "b": {"version": "1.0.0"}
            }
        }"#,
        )
        .unwrap();
        let pkg = pkg(r#"{"dependencies": {"a": "^1.0.0"}}"#);
        let graph = lock.dep_graph(&pkg).unwrap();
        assert_eq!(graph.roots.len(), 1);
        let a = &graph.nodes[graph.roots[0].node];
        assert_eq!((a.name.as_str(), a.version.as_str()), ("a", "1.0.0"));
        // nested b@2.5.0 wins over top-level b@1.0.0.
        let b = &graph.nodes[a.deps[0].node];
        assert_eq!((b.name.as_str(), b.version.as_str()), ("b", "2.5.0"));
    }
}
