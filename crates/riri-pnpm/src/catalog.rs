//! pnpm catalog support (pnpm-workspace.yaml).
//!
//! Reads the `catalog:` (default) and `catalogs:<name>:` (named) maps
//! from `pnpm-workspace.yaml`, and provides targeted line-edit for
//! rewriting a single entry while preserving comments + key order.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use riri_find_up::find_up;

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("invalid YAML: {0}")]
    Yaml(#[from] serde_saphyr::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogEditError {
    #[error("catalog block not found in pnpm-workspace.yaml")]
    BlockNotFound,
    #[error("key `{0}` not found in catalog block")]
    KeyNotFound(String),
    #[error("entry `{0}` uses an unsupported multiline yaml value")]
    Multiline(String),
}

#[derive(Debug, Clone)]
pub struct PnpmCatalog {
    pub default: BTreeMap<String, String>,
    pub named: BTreeMap<String, BTreeMap<String, String>>,
    pub raw: String,
}

/// Walks up from `start` looking for `pnpm-workspace.yaml`. Returns the path
/// if found.
#[must_use]
pub fn find_workspace_yaml(start: &Path) -> Option<PathBuf> {
    find_up(start, &["pnpm-workspace.yaml"]).into_iter().next()
}

fn line_indent(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

fn unquote_key(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'')
            || (bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"'))
    {
        &raw[1..raw.len() - 1]
    } else {
        raw
    }
}

/// Returns `(key_raw, value_start, value_end)` byte offsets into `line` for
/// a `<indent><key>: <value>[<trailing>]` line, or `None` if it does not match.
fn split_kv(line: &str) -> Option<(&str, usize, usize)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let indent = line.len() - trimmed.len();

    // Find the key terminator `:`.
    let mut chars = trimmed.char_indices();
    let mut key_end = None;
    let mut in_single = false;
    let mut in_double = false;
    for (offset, ch) in &mut chars {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ':' if !in_single && !in_double => {
                key_end = Some(offset);
                break;
            }
            _ => {}
        }
    }
    let key_end = key_end?;
    let key = trimmed[..key_end].trim_end();

    let after_colon = key_end + ':'.len_utf8();
    let value_part = &trimmed[after_colon..];
    let value_offset_in_trimmed = after_colon + (value_part.len() - value_part.trim_start().len());
    let value_start_in_trimmed = value_offset_in_trimmed;

    // Find value end: stop before unescaped `#` (after at least one space) or EOL.
    // `prev_space = true` initially because `value_str` starts after `trim_start`
    // of the value section: either at least one space was consumed (so the char
    // before `value_str[0]` is a space), or the value section is empty.
    let value_str = &trimmed[value_start_in_trimmed..];
    let mut in_single = false;
    let mut in_double = false;
    let mut value_end_in_value = value_str.len();
    let mut prev_space = true;
    for (offset, ch) in value_str.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double && prev_space => {
                value_end_in_value = offset;
                break;
            }
            _ => {}
        }
        prev_space = ch == ' ' || ch == '\t';
    }
    let value_end_in_trimmed =
        value_start_in_trimmed + value_str[..value_end_in_value].trim_end().len();

    Some((
        key,
        indent + value_start_in_trimmed,
        indent + value_end_in_trimmed,
    ))
}

fn value_needs_quoting(value: &str) -> bool {
    value.is_empty()
        || value.chars().any(|c| {
            matches!(
                c,
                ':' | '#'
                    | '?'
                    | ','
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '&'
                    | '*'
                    | '!'
                    | '|'
                    | '>'
                    | '\''
                    | '"'
                    | '%'
                    | '@'
                    | '`'
            )
        })
        || value.starts_with('-')
        || value.starts_with(char::is_whitespace)
}

fn quote_value(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Locates the body indent + line range `[start_line_idx, end_line_idx)` for a
/// catalog block.
fn locate_block(lines: &[&str], catalog_name: Option<&str>) -> Option<(usize, usize, usize)> {
    let header = match catalog_name {
        None => "catalog:",
        Some(_) => "catalogs:",
    };

    let header_line = lines.iter().position(|l| l.trim_end() == header)?;

    if let Some(name) = catalog_name {
        // Find `<indent>name:` under `catalogs:`.
        let catalogs_indent = line_indent(lines[header_line]);
        let mut name_line = None;
        for (idx, line) in lines.iter().enumerate().skip(header_line + 1) {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            let indent = line_indent(line);
            if indent <= catalogs_indent {
                break;
            }
            if trimmed.trim_start() == format!("{name}:") {
                name_line = Some(idx);
                break;
            }
        }
        let name_line = name_line?;
        let name_indent = line_indent(lines[name_line]);
        let body_start = name_line + 1;
        let mut body_indent = None;
        let mut body_end = lines.len();
        for (idx, line) in lines.iter().enumerate().skip(body_start) {
            if line.trim().is_empty() {
                continue;
            }
            let indent = line_indent(line);
            if indent <= name_indent {
                body_end = idx;
                break;
            }
            if body_indent.is_none() {
                body_indent = Some(indent);
            }
        }
        Some((body_indent?, body_start, body_end))
    } else {
        let header_indent = line_indent(lines[header_line]);
        let body_start = header_line + 1;
        let mut body_indent = None;
        let mut body_end = lines.len();
        for (idx, line) in lines.iter().enumerate().skip(body_start) {
            if line.trim().is_empty() {
                continue;
            }
            let indent = line_indent(line);
            if indent <= header_indent {
                body_end = idx;
                break;
            }
            if body_indent.is_none() {
                body_indent = Some(indent);
            }
        }
        Some((body_indent?, body_start, body_end))
    }
}

impl PnpmCatalog {
    /// Parse a `pnpm-workspace.yaml` content string.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::Yaml`] if the YAML is invalid.
    pub fn parse(yaml: &str) -> Result<Self, CatalogError> {
        #[derive(Debug, serde::Deserialize, Default)]
        struct Root {
            #[serde(default)]
            catalog: BTreeMap<String, String>,
            #[serde(default)]
            catalogs: BTreeMap<String, BTreeMap<String, String>>,
        }

        if yaml.trim().is_empty() {
            return Ok(Self {
                default: BTreeMap::new(),
                named: BTreeMap::new(),
                raw: yaml.to_string(),
            });
        }

        let root: Root = serde_saphyr::from_str(yaml)?;
        Ok(Self {
            default: root.catalog,
            named: root.catalogs,
            raw: yaml.to_string(),
        })
    }

    /// Resolves a `catalog:` (when `catalog_name` is `None`) or
    /// `catalog:<name>` reference to the entry value.
    #[must_use]
    pub fn find_entry(&self, dep: &str, catalog_name: Option<&str>) -> Option<&str> {
        match catalog_name {
            None => self.default.get(dep).map(String::as_str),
            Some(name) => self
                .named
                .get(name)
                .and_then(|m| m.get(dep))
                .map(String::as_str),
        }
    }

    /// Rewrite a single catalog entry in `raw` and return the new content.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogEditError`] if the catalog block or the key cannot
    /// be located, or if the existing entry uses an unsupported multiline
    /// YAML form.
    pub fn edit_line(
        raw: &str,
        catalog_name: Option<&str>,
        dep: &str,
        new_value: &str,
    ) -> Result<String, CatalogEditError> {
        let lines: Vec<&str> = raw.split_inclusive('\n').collect();
        let line_refs: Vec<&str> = lines.iter().map(|l| l.trim_end_matches('\n')).collect();

        let (body_indent, body_start, body_end) =
            locate_block(&line_refs, catalog_name).ok_or(CatalogEditError::BlockNotFound)?;

        for idx in body_start..body_end {
            let line = line_refs[idx];
            if line_indent(line) != body_indent {
                continue;
            }
            let Some((raw_key, val_start, val_end)) = split_kv(line) else {
                continue;
            };
            if unquote_key(raw_key) != dep {
                continue;
            }

            let value_slice = &line[val_start..val_end];
            if matches!(value_slice, ">" | "|" | ">-" | "|-" | ">+" | "|+")
                || value_slice.starts_with('[')
                || value_slice.starts_with('{')
            {
                return Err(CatalogEditError::Multiline(dep.to_string()));
            }

            let new_token = if value_needs_quoting(new_value) {
                quote_value(new_value)
            } else {
                new_value.to_string()
            };

            // When the existing value is empty and the trailing region begins
            // with `#`, the leading space that separated the (empty) value from
            // the comment was consumed by `trim_start` in `split_kv`. Inject a
            // space so the new value is not glued to the comment marker.
            let trailing = &line[val_end..];
            let separator = if value_slice.is_empty() && trailing.starts_with('#') {
                " "
            } else {
                ""
            };
            let new_line = format!(
                "{}{}{}{}",
                &line[..val_start],
                new_token,
                separator,
                trailing,
            );
            let mut out = String::with_capacity(raw.len() + new_token.len());
            for (i, original) in lines.iter().enumerate() {
                if i == idx {
                    out.push_str(&new_line);
                    let original_with_newline = lines[idx];
                    let newline_len = original_with_newline.len() - line.len();
                    out.push_str(
                        &original_with_newline[original_with_newline.len() - newline_len..],
                    );
                } else {
                    out.push_str(original);
                }
            }
            return Ok(out);
        }

        Err(CatalogEditError::KeyNotFound(dep.to_string()))
    }
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn parses_default_catalog() {
        let yaml = r#"
catalog:
  react: ^18.0.0
  lodash: "^4.17.21"
"#;
        let catalog = PnpmCatalog::parse(yaml).expect("parse");
        assert_eq!(
            catalog.default.get("react").map(String::as_str),
            Some("^18.0.0")
        );
        assert_eq!(
            catalog.default.get("lodash").map(String::as_str),
            Some("^4.17.21")
        );
        assert!(catalog.named.is_empty());
    }

    #[test]
    fn parses_named_catalogs() {
        let yaml = r"
catalogs:
  vue3:
    vue: ^3.4.0
    pinia: ^2.1.0
  react17:
    react: ^17.0.2
";
        let catalog = PnpmCatalog::parse(yaml).expect("parse");
        assert_eq!(
            catalog.named["vue3"].get("vue").map(String::as_str),
            Some("^3.4.0")
        );
        assert_eq!(
            catalog.named["vue3"].get("pinia").map(String::as_str),
            Some("^2.1.0")
        );
        assert_eq!(
            catalog.named["react17"].get("react").map(String::as_str),
            Some("^17.0.2")
        );
        assert!(catalog.default.is_empty());
    }

    #[test]
    fn parses_both_default_and_named() {
        let yaml = r"
catalog:
  react: ^18.0.0
catalogs:
  vue3:
    vue: ^3.4.0
";
        let catalog = PnpmCatalog::parse(yaml).expect("parse");
        assert_eq!(
            catalog.default.get("react").map(String::as_str),
            Some("^18.0.0")
        );
        assert_eq!(
            catalog.named["vue3"].get("vue").map(String::as_str),
            Some("^3.4.0")
        );
    }

    #[test]
    fn parses_scoped_package_names() {
        let yaml = r#"
catalog:
  '@scope/pkg': ^1.0.0
  "@other/pkg": ^2.0.0
"#;
        let catalog = PnpmCatalog::parse(yaml).expect("parse");
        assert_eq!(
            catalog.default.get("@scope/pkg").map(String::as_str),
            Some("^1.0.0")
        );
        assert_eq!(
            catalog.default.get("@other/pkg").map(String::as_str),
            Some("^2.0.0")
        );
    }

    #[test]
    fn find_entry_default_vs_named() {
        let yaml = r"
catalog:
  react: ^18.0.0
catalogs:
  vue3:
    vue: ^3.4.0
";
        let catalog = PnpmCatalog::parse(yaml).expect("parse");
        assert_eq!(catalog.find_entry("react", None), Some("^18.0.0"));
        assert_eq!(catalog.find_entry("vue", Some("vue3")), Some("^3.4.0"));
        assert_eq!(catalog.find_entry("missing", None), None);
        assert_eq!(catalog.find_entry("react", Some("vue3")), None);
    }

    #[test]
    fn preserves_raw_content() {
        let yaml = "catalog:\n  react: ^18.0.0\n";
        let catalog = PnpmCatalog::parse(yaml).expect("parse");
        assert_eq!(catalog.raw, yaml);
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    #[test]
    fn edits_unquoted_value_in_default_catalog() {
        let yaml = "catalog:\n  react: ^18.0.0\n  lodash: ^4.17.21\n";
        let out = PnpmCatalog::edit_line(yaml, None, "react", "18.2.0").expect("edit");
        assert_eq!(out, "catalog:\n  react: 18.2.0\n  lodash: ^4.17.21\n");
    }

    #[test]
    fn edits_double_quoted_value_preserving_trailing_comment() {
        let yaml = "catalog:\n  lodash: \"^4.17.21\" # pinned for security\n";
        let out = PnpmCatalog::edit_line(yaml, None, "lodash", "4.17.25").expect("edit");
        assert_eq!(out, "catalog:\n  lodash: 4.17.25 # pinned for security\n");
    }

    #[test]
    fn edits_single_quoted_scoped_key() {
        let yaml = "catalog:\n  '@scope/pkg': ^1.0.0\n";
        let out = PnpmCatalog::edit_line(yaml, None, "@scope/pkg", "1.1.0").expect("edit");
        assert_eq!(out, "catalog:\n  '@scope/pkg': 1.1.0\n");
    }

    #[test]
    fn edits_entry_in_named_catalog() {
        let yaml = "catalogs:\n  vue3:\n    vue: ^3.4.0\n    pinia: ^2.1.0\n";
        let out = PnpmCatalog::edit_line(yaml, Some("vue3"), "pinia", "2.1.7").expect("edit");
        assert_eq!(
            out,
            "catalogs:\n  vue3:\n    vue: ^3.4.0\n    pinia: 2.1.7\n"
        );
    }

    #[test]
    fn quotes_new_value_when_it_contains_special_chars() {
        let yaml = "catalog:\n  react: ^18.0.0\n";
        let out = PnpmCatalog::edit_line(yaml, None, "react", ">=18.0.0").expect("edit");
        assert_eq!(out, "catalog:\n  react: \">=18.0.0\"\n");
    }

    #[test]
    fn errors_when_catalog_block_missing() {
        let yaml = "packages:\n  - apps/*\n";
        let err = PnpmCatalog::edit_line(yaml, None, "react", "18.2.0").expect_err("err");
        assert!(matches!(err, CatalogEditError::BlockNotFound));
    }

    #[test]
    fn errors_when_named_catalog_missing() {
        let yaml = "catalogs:\n  vue3:\n    vue: ^3.4.0\n";
        let err =
            PnpmCatalog::edit_line(yaml, Some("react17"), "react", "17.0.2").expect_err("err");
        assert!(matches!(err, CatalogEditError::BlockNotFound));
    }

    #[test]
    fn errors_when_key_missing() {
        let yaml = "catalog:\n  react: ^18.0.0\n";
        let err = PnpmCatalog::edit_line(yaml, None, "lodash", "4.17.21").expect_err("err");
        match err {
            CatalogEditError::KeyNotFound(name) => assert_eq!(name, "lodash"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn errors_on_multiline_value() {
        let yaml = "catalog:\n  react: >\n    ^18.0.0\n";
        let err = PnpmCatalog::edit_line(yaml, None, "react", "18.2.0").expect_err("err");
        assert!(matches!(err, CatalogEditError::Multiline(_)));
    }

    #[test]
    fn preserves_trailing_comment_when_value_is_empty() {
        // Currently this yaml is unusual (empty value = null), but if we are asked
        // to overwrite it, the trailing comment must survive.
        let yaml = "catalog:\n  react: # pinned\n";
        let out = PnpmCatalog::edit_line(yaml, None, "react", "18.2.0").expect("edit");
        assert_eq!(out, "catalog:\n  react: 18.2.0 # pinned\n");
    }
}

#[cfg(test)]
mod find_tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_yaml_in_start_dir() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        fs::write(tmp.path().join("pnpm-workspace.yaml"), "catalog: {}\n").expect("write");
        let found = find_workspace_yaml(tmp.path()).expect("found");
        assert_eq!(found, tmp.path().join("pnpm-workspace.yaml"));
    }

    #[test]
    fn finds_yaml_in_parent_dir() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        fs::write(tmp.path().join("pnpm-workspace.yaml"), "catalog: {}\n").expect("write");
        let nested = tmp.path().join("apps/web");
        fs::create_dir_all(&nested).expect("nested");
        let found = find_workspace_yaml(&nested).expect("found");
        assert_eq!(found, tmp.path().join("pnpm-workspace.yaml"));
    }

    #[test]
    fn returns_none_when_absent() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        assert!(find_workspace_yaml(tmp.path()).is_none());
    }
}
