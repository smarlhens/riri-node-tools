use criterion::{Criterion, criterion_group, criterion_main};
use riri_common::{LockGraph, LockfileGraph, PackageJson};
use riri_ncd::analyze::analyze;
use riri_ncd::{DeprecatedField, DeprecationSource, Packument, PackumentVersion, SourceError};
use riri_npm::NpmPackageLock;
use std::collections::HashMap;
use std::hint::black_box;

/// In-memory packument source built from a graph, marking every 10th node
/// deprecated — keeps benches network-free and deterministic.
struct MapSource(HashMap<String, Packument>);

impl DeprecationSource for MapSource {
    fn packument(&self, name: &str) -> Result<Option<Packument>, SourceError> {
        Ok(self.0.get(name).cloned())
    }
}

fn load_fixture(name: &str) -> (NpmPackageLock, PackageJson) {
    let base = format!("../../fixtures/{name}");
    let lock_content = std::fs::read_to_string(format!("{base}/package-lock.json"))
        .unwrap_or_else(|e| panic!("failed to read lockfile for {name}: {e}"));
    let pkg_content = std::fs::read_to_string(format!("{base}/package.json"))
        .unwrap_or_else(|e| panic!("failed to read package.json for {name}: {e}"));
    let lock = NpmPackageLock::parse(&lock_content)
        .unwrap_or_else(|e| panic!("failed to parse {name}: {e}"));
    let pkg: PackageJson = serde_json::from_str(&pkg_content)
        .unwrap_or_else(|e| panic!("failed to parse {name} package.json: {e}"));
    (lock, pkg)
}

fn build_source(graph: &LockGraph) -> MapSource {
    let mut map: HashMap<String, Packument> = HashMap::new();
    for (i, node) in graph.nodes.iter().enumerate() {
        let pack = map.entry(node.name.clone()).or_default();
        let version = if i % 10 == 0 {
            PackumentVersion {
                deprecated: Some(DeprecatedField::Message("deprecated for bench".into())),
                ..PackumentVersion::default()
            }
        } else {
            PackumentVersion::default()
        };
        pack.versions.insert(node.version.clone(), version);
        pack.dist_tags
            .entry("latest".to_string())
            .or_insert_with(|| node.version.clone());
    }
    MapSource(map)
}

fn bench_graph_500(c: &mut Criterion) {
    let (lock, pkg) = load_fixture("npd-npm-v3-500-deps");
    c.bench_function("check_deprecations: graph 500 deps", |b| {
        b.iter(|| black_box(lock.dep_graph(&pkg).expect("graph")));
    });
}

fn bench_analyze_500(c: &mut Criterion) {
    let (lock, pkg) = load_fixture("npd-npm-v3-500-deps");
    let graph = lock.dep_graph(&pkg).expect("graph");
    let source = build_source(&graph);
    c.bench_function("check_deprecations: analyze 500 deps", |b| {
        b.iter(|| black_box(analyze(&graph, "bench", &source).expect("analyze")));
    });
}

criterion_group!(benches, bench_graph_500, bench_analyze_500);
criterion_main!(benches);
