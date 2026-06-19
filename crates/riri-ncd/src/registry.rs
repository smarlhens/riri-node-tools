//! Registry packument client and parallel fetch.

use crate::{DeprecationSource, Packument, SourceError};
use riri_common::NpmrcRegistryConfig;
use std::collections::HashMap;
use std::sync::Mutex;

const ACCEPT_ABBREVIATED: &str = "application/vnd.npm.install-v1+json";
const CONCURRENCY: usize = 16;

/// `https://host/path/{name}` with the scoped-name slash percent-encoded.
#[must_use]
pub fn packument_url(registry: &str, name: &str) -> String {
    let encoded = name.replace('/', "%2F");
    format!("{}/{}", registry.trim_end_matches('/'), encoded)
}

/// Live registry-backed [`DeprecationSource`].
pub struct RegistryClient {
    agent: ureq::Agent,
    config: NpmrcRegistryConfig,
    /// Overrides every scope when set (`--registry` flag).
    registry_override: Option<String>,
}

impl RegistryClient {
    #[must_use]
    pub fn new(config: NpmrcRegistryConfig, registry_override: Option<String>) -> Self {
        Self {
            agent: ureq::Agent::new_with_defaults(),
            config,
            registry_override,
        }
    }

    fn registry_for(&self, name: &str) -> &str {
        self.registry_override.as_deref().map_or_else(
            || self.config.registry_for(name),
            |r| r.trim_end_matches('/'),
        )
    }
}

/// Read a packument from a `file://` registry directory (`{dir}/{name}.json`).
/// Offline source used by fixtures and README generation. `Ok(None)` when the
/// file is absent, mirroring a registry 404.
fn read_packument_file(dir: &str, name: &str) -> Result<Option<Packument>, SourceError> {
    let path = std::path::Path::new(dir).join(format!("{name}.json"));
    match std::fs::read_to_string(&path) {
        Ok(body) => serde_json::from_str(&body)
            .map(Some)
            .map_err(|e| SourceError::Request {
                name: name.to_string(),
                detail: format!("invalid packument: {e}"),
            }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(SourceError::Request {
            name: name.to_string(),
            detail: e.to_string(),
        }),
    }
}

impl DeprecationSource for RegistryClient {
    fn packument(&self, name: &str) -> Result<Option<Packument>, SourceError> {
        let registry = self.registry_for(name);
        if let Some(dir) = registry.strip_prefix("file://") {
            return read_packument_file(dir, name);
        }
        let url = packument_url(registry, name);
        let mut req = self.agent.get(&url).header("accept", ACCEPT_ABBREVIATED);
        if let Some(token) = self.config.token_for(registry) {
            req = req.header("authorization", &format!("Bearer {token}"));
        }
        match req.call() {
            Ok(mut res) => {
                let body = res
                    .body_mut()
                    .read_to_string()
                    .map_err(|e| SourceError::Request {
                        name: name.to_string(),
                        detail: e.to_string(),
                    })?;
                serde_json::from_str(&body)
                    .map(Some)
                    .map_err(|e| SourceError::Request {
                        name: name.to_string(),
                        detail: format!("invalid packument: {e}"),
                    })
            }
            Err(ureq::Error::StatusCode(404)) => Ok(None),
            Err(e) => Err(SourceError::Request {
                name: name.to_string(),
                detail: e.to_string(),
            }),
        }
    }
}

/// Fetch all names concurrently. Unknown packages are omitted from the map;
/// hard failures are collected so the caller can abort with context.
#[must_use]
pub fn fetch_all(
    source: &dyn DeprecationSource,
    names: &[String],
) -> (HashMap<String, Packument>, Vec<SourceError>) {
    let queue = Mutex::new(names.to_vec());
    let results = Mutex::new(HashMap::new());
    let errors = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..CONCURRENCY.min(names.len().max(1)) {
            s.spawn(|| {
                loop {
                    let Some(name) = queue.lock().map_or(None, |mut q| q.pop()) else {
                        return;
                    };
                    match source.packument(&name) {
                        Ok(Some(p)) => {
                            if let Ok(mut r) = results.lock() {
                                r.insert(name, p);
                            }
                        }
                        Ok(None) => tracing::debug!(
                            target: "riri_ncd::registry",
                            package = %name,
                            "not found on registry"
                        ),
                        Err(e) => {
                            if let Ok(mut errs) = errors.lock() {
                                errs.push(e);
                            }
                        }
                    }
                }
            });
        }
    });
    (
        results.into_inner().unwrap_or_default(),
        errors.into_inner().unwrap_or_default(),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn packument_url_encodes_scoped_names() {
        assert_eq!(
            packument_url("https://registry.npmjs.org", "@scope/name"),
            "https://registry.npmjs.org/@scope%2Fname"
        );
        assert_eq!(
            packument_url("https://r.example.com/npm", "lodash"),
            "https://r.example.com/npm/lodash"
        );
    }

    #[test]
    fn file_registry_reads_local_packuments() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("foo.json"),
            r#"{"dist-tags": {"latest": "1.0.0"}, "versions": {"1.0.0": {}}}"#,
        )
        .unwrap();
        let client = RegistryClient::new(
            riri_common::NpmrcRegistryConfig::default(),
            Some(format!("file://{}", dir.path().display())),
        );
        let packument = client.packument("foo").unwrap().unwrap();
        assert_eq!(packument.latest(), Some("1.0.0"));
        // Absent file behaves like a registry 404.
        assert!(client.packument("missing").unwrap().is_none());
    }

    #[test]
    fn fetch_all_collects_results_and_reports_failures() {
        struct Stub;
        impl DeprecationSource for Stub {
            fn packument(&self, name: &str) -> Result<Option<Packument>, SourceError> {
                match name {
                    "missing" => Ok(None),
                    "broken" => Err(SourceError::Request {
                        name: name.into(),
                        detail: "boom".into(),
                    }),
                    _ => Ok(Some(Packument::default())),
                }
            }
        }
        let names: Vec<String> = vec!["a".into(), "missing".into(), "broken".into()];
        let (map, errors) = fetch_all(&Stub, &names);
        assert!(map.contains_key("a"));
        assert!(!map.contains_key("missing"));
        assert_eq!(errors.len(), 1);
    }
}
