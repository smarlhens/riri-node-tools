//! `.npmrc` helpers shared by `nce` (`engine-strict=true`), `npd`
//! (`save-exact=true`), and `ncd`: write-side upsert and read-side registry/auth
//! resolution.

#[cfg(feature = "graph")]
use std::collections::HashMap;
use std::path::Path;

#[cfg(feature = "graph")]
pub const DEFAULT_REGISTRY: &str = "https://registry.npmjs.org";

/// Read-side `.npmrc` config: default + scoped registries and auth tokens.
/// Home config is read first, project config overrides per key.
#[cfg(feature = "graph")]
#[derive(Debug, Clone, Default)]
pub struct NpmrcRegistryConfig {
    default_registry: Option<String>,
    /// `@scope` (with the `@`) → registry URL (no trailing slash).
    scoped: HashMap<String, String>,
    /// `//host/path` fragment (no trailing slash, no `:_authToken` suffix) → token.
    tokens: HashMap<String, String>,
}

#[cfg(feature = "graph")]
impl NpmrcRegistryConfig {
    /// Read `{home}/.npmrc` then `{project_dir}/.npmrc` (project wins).
    ///
    /// `home` defaults to the `HOME` env var when `None`.
    #[must_use]
    pub fn read(project_dir: &Path, home: Option<&Path>) -> Self {
        Self::read_with_env(project_dir, home, &|key| std::env::var(key).ok())
    }

    /// Like [`Self::read`], but resolves `${VAR}` token references through `env`
    /// instead of the process environment. Lets tests inject values without
    /// mutating global state (no `unsafe { std::env::set_var }`).
    fn read_with_env(
        project_dir: &Path,
        home: Option<&Path>,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Self {
        let mut cfg = Self::default();
        let home_dir = home
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("HOME").map(Into::into));
        if let Some(h) = home_dir {
            cfg.merge_file(&h.join(".npmrc"), env);
        }
        cfg.merge_file(&project_dir.join(".npmrc"), env);
        cfg
    }

    fn merge_file(&mut self, path: &Path, env: &dyn Fn(&str) -> Option<String>) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), expand_env(value.trim(), env));
            if key == "registry" {
                self.default_registry = Some(strip_trailing_slash(&value));
            } else if let Some(scope) = key.strip_suffix(":registry") {
                if scope.starts_with('@') {
                    self.scoped
                        .insert(scope.to_string(), strip_trailing_slash(&value));
                }
            } else if let Some(fragment) = key.strip_suffix(":_authToken")
                && fragment.starts_with("//")
            {
                self.tokens.insert(strip_trailing_slash(fragment), value);
            }
        }
    }

    /// Registry URL (no trailing slash) for a package name, honoring scopes.
    #[must_use]
    pub fn registry_for(&self, package_name: &str) -> &str {
        if let Some(scope) = package_name
            .split('/')
            .next()
            .filter(|s| s.starts_with('@'))
            && let Some(reg) = self.scoped.get(scope)
        {
            return reg;
        }
        self.default_registry.as_deref().unwrap_or(DEFAULT_REGISTRY)
    }

    /// Auth token whose `//host/path` fragment matches the registry URL.
    #[must_use]
    pub fn token_for(&self, registry_url: &str) -> Option<&str> {
        let without_scheme = registry_url
            .split_once("//")
            .map_or(registry_url, |(_, rest)| rest);
        let fragment = format!(
            "//{}",
            strip_trailing_slash(without_scheme).trim_start_matches('/')
        );
        self.tokens.get(&fragment).map(String::as_str)
    }
}

#[cfg(feature = "graph")]
fn strip_trailing_slash(s: &str) -> String {
    s.trim_end_matches('/').to_string()
}

/// Expand `${VAR}` references via `env`; unknown vars expand empty.
#[cfg(feature = "graph")]
fn expand_env(value: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find('}') {
            out.push_str(&env(&after[..end]).unwrap_or_default());
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[start..]);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

/// Outcome of a [`upsert_npmrc_flag`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpmrcOutcome {
    /// File already contained the line; nothing was written.
    AlreadySet,
    /// File was created or appended with the line.
    Added,
}

/// Ensures `dir/.npmrc` contains the literal `flag` line.
///
/// Behaviour:
///   - missing file → create with `flag\n`
///   - existing content already contains `flag` (substring) → no-op
///   - else → append `flag\n` (with a leading newline if the file did not end with one)
///
/// # Errors
///
/// Returns the underlying [`std::io::Error`] when reading or writing fails.
pub fn upsert_npmrc_flag(dir: &Path, flag: &str) -> std::io::Result<NpmrcOutcome> {
    let path = dir.join(".npmrc");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.contains(flag) {
        return Ok(NpmrcOutcome::AlreadySet);
    }
    let new_content = if existing.is_empty() {
        format!("{flag}\n")
    } else if existing.ends_with('\n') {
        format!("{existing}{flag}\n")
    } else {
        format!("{existing}\n{flag}\n")
    };
    std::fs::write(&path, new_content)?;
    Ok(NpmrcOutcome::Added)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn creates_file_when_missing() {
        let tmp = TempDir::new().unwrap();
        let outcome = upsert_npmrc_flag(tmp.path(), "engine-strict=true").unwrap();
        assert_eq!(outcome, NpmrcOutcome::Added);
        let content = std::fs::read_to_string(tmp.path().join(".npmrc")).unwrap();
        assert_eq!(content, "engine-strict=true\n");
    }

    #[test]
    fn no_op_when_flag_already_present() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".npmrc"), "engine-strict=true\n").unwrap();
        let outcome = upsert_npmrc_flag(tmp.path(), "engine-strict=true").unwrap();
        assert_eq!(outcome, NpmrcOutcome::AlreadySet);
    }

    #[test]
    fn appends_when_other_lines_present() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".npmrc"), "registry=https://example.com\n").unwrap();
        let outcome = upsert_npmrc_flag(tmp.path(), "save-exact=true").unwrap();
        assert_eq!(outcome, NpmrcOutcome::Added);
        let content = std::fs::read_to_string(tmp.path().join(".npmrc")).unwrap();
        assert_eq!(content, "registry=https://example.com\nsave-exact=true\n");
    }

    #[test]
    fn appends_with_newline_when_existing_lacks_trailing_newline() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".npmrc"), "registry=https://example.com").unwrap();
        let outcome = upsert_npmrc_flag(tmp.path(), "save-exact=true").unwrap();
        assert_eq!(outcome, NpmrcOutcome::Added);
        let content = std::fs::read_to_string(tmp.path().join(".npmrc")).unwrap();
        assert_eq!(content, "registry=https://example.com\nsave-exact=true\n");
    }

    #[cfg(feature = "graph")]
    #[test]
    fn reads_default_registry_and_scoped_registry() {
        let home = TempDir::new().unwrap();
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(".npmrc"),
            "registry=https://nexus.example.com/repository/npm/\n@acme:registry=https://nexus.example.com/repository/acme/\n",
        )
        .unwrap();
        let cfg = NpmrcRegistryConfig::read(tmp.path(), Some(home.path()));
        assert_eq!(
            cfg.registry_for("lodash"),
            "https://nexus.example.com/repository/npm"
        );
        assert_eq!(
            cfg.registry_for("@acme/ui"),
            "https://nexus.example.com/repository/acme"
        );
    }

    #[cfg(feature = "graph")]
    #[test]
    fn default_registry_is_npmjs_when_no_npmrc() {
        let home = TempDir::new().unwrap();
        let tmp = TempDir::new().unwrap();
        let cfg = NpmrcRegistryConfig::read(tmp.path(), Some(home.path()));
        assert_eq!(cfg.registry_for("lodash"), "https://registry.npmjs.org");
    }

    #[cfg(feature = "graph")]
    #[test]
    fn project_npmrc_overrides_home_npmrc() {
        let home = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        std::fs::write(
            home.path().join(".npmrc"),
            "registry=https://home.example.com/\n",
        )
        .unwrap();
        std::fs::write(
            proj.path().join(".npmrc"),
            "registry=https://proj.example.com/\n",
        )
        .unwrap();
        let cfg = NpmrcRegistryConfig::read(proj.path(), Some(home.path()));
        assert_eq!(cfg.registry_for("x"), "https://proj.example.com");
    }

    #[cfg(feature = "graph")]
    #[test]
    fn token_for_matches_registry_host_path() {
        let home = TempDir::new().unwrap();
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(".npmrc"),
            "registry=https://nexus.example.com/repository/npm/\n//nexus.example.com/repository/npm/:_authToken=abc123\n",
        )
        .unwrap();
        let cfg = NpmrcRegistryConfig::read(tmp.path(), Some(home.path()));
        assert_eq!(
            cfg.token_for("https://nexus.example.com/repository/npm"),
            Some("abc123")
        );
        assert_eq!(cfg.token_for("https://registry.npmjs.org"), None);
    }

    #[cfg(feature = "graph")]
    #[test]
    fn expands_env_vars_in_token() {
        let home = TempDir::new().unwrap();
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(".npmrc"),
            "//registry.npmjs.org/:_authToken=${NPMRC_TEST_TOKEN}\n",
        )
        .unwrap();
        let cfg = NpmrcRegistryConfig::read_with_env(tmp.path(), Some(home.path()), &|key| {
            (key == "NPMRC_TEST_TOKEN").then(|| "sekret".to_string())
        });
        assert_eq!(cfg.token_for("https://registry.npmjs.org"), Some("sekret"));
    }

    #[cfg(feature = "graph")]
    #[test]
    fn home_token_visible_when_project_has_none() {
        let home = TempDir::new().unwrap();
        let proj = TempDir::new().unwrap();
        std::fs::write(
            home.path().join(".npmrc"),
            "//registry.npmjs.org/:_authToken=fromhome\n",
        )
        .unwrap();
        std::fs::write(
            proj.path().join(".npmrc"),
            "registry=https://proj.example.com/\n",
        )
        .unwrap();
        let cfg = NpmrcRegistryConfig::read(proj.path(), Some(home.path()));
        assert_eq!(
            cfg.token_for("https://registry.npmjs.org"),
            Some("fromhome")
        );
    }

    #[cfg(feature = "graph")]
    #[test]
    fn expands_multiple_env_vars_in_one_value() {
        let home = TempDir::new().unwrap();
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(".npmrc"),
            "//r.example.com/:_authToken=${NCD_T_A}-${NCD_T_B}\n",
        )
        .unwrap();
        let cfg =
            NpmrcRegistryConfig::read_with_env(tmp.path(), Some(home.path()), &|key| match key {
                "NCD_T_A" => Some("a".to_string()),
                "NCD_T_B" => Some("b".to_string()),
                _ => None,
            });
        assert_eq!(cfg.token_for("https://r.example.com"), Some("a-b"));
    }
}
