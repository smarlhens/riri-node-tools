//! Text rendering of the pruned dependency tree.

use crate::analyze::RenderNode;
use console::style;
use std::fmt::Write as _;

/// Render a pruned dependency tree to a string (one node per line).
#[must_use]
pub fn render_tree(root: &RenderNode) -> String {
    let mut out = String::new();
    out.push_str(&label(root));
    out.push('\n');
    render_children(&root.children, "", &mut out);
    out
}

fn render_children(children: &[RenderNode], prefix: &str, out: &mut String) {
    let last_index = children.len().saturating_sub(1);
    for (i, child) in children.iter().enumerate() {
        let last = i == last_index;
        out.push_str(prefix);
        out.push_str(if last { "└─ " } else { "├─ " });
        out.push_str(&label(child));
        out.push('\n');
        let child_prefix = format!("{prefix}{}", if last { "   " } else { "│  " });
        render_children(&child.children, &child_prefix, out);
    }
}

/// Assemble a node's single-line label. Styling is a no-op when colors are
/// disabled, so the segment text matches exactly.
fn label(node: &RenderNode) -> String {
    let mut s = match &node.version {
        Some(v) => format!("{}@{v}", node.name),
        None => node.name.clone(),
    };
    if let Some(kind) = node.kind {
        let _ = write!(s, " ({kind})");
    }
    if let Some(fix) = &node.fix {
        // A node can carry several `; `-joined labels (e.g. a package that is
        // both replacement-needed and a blocker). Render each on its own so a
        // blocker segment keeps its ⛔ even when combined with another label.
        for segment in fix.split("; ") {
            if segment.starts_with("blocks:") {
                let _ = write!(s, "  {} {segment}", style("⛔").red());
            } else {
                let _ = write!(s, "  ({segment})");
            }
        }
    }
    if let Some(message) = &node.deprecated {
        let warn = style("⚠").yellow();
        if message.is_empty() {
            let _ = write!(s, "  {warn} deprecated");
        } else {
            let _ = write!(s, "  {warn} deprecated: {message}");
        }
    }
    if let Some(latest) = &node.latest {
        let _ = write!(s, "  {}", style(format!("(latest: {latest})")).dim());
    }
    if node.deduped {
        s.push_str(" (see above)");
    }
    if node.circular {
        let _ = write!(s, "  {} circular", style("↻").dim());
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn leaf(name: &str, version: &str) -> RenderNode {
        RenderNode {
            name: name.into(),
            version: Some(version.into()),
            kind: None,
            latest: None,
            deprecated: None,
            fix: None,
            circular: false,
            deduped: false,
            children: Vec::new(),
        }
    }

    #[test]
    fn renders_pruned_tree_with_annotations() {
        console::set_colors_enabled(false);
        let tree = RenderNode {
            name: "my-project".into(),
            version: None,
            kind: None,
            latest: None,
            deprecated: None,
            fix: None,
            circular: false,
            deduped: false,
            children: vec![
                RenderNode {
                    latest: Some("5.2.0".into()),
                    children: vec![RenderNode {
                        fix: Some(
                            "blocks: requires foo@~1.0.0, fix needs 2.1.0 → bar update required"
                                .into(),
                        ),
                        children: vec![RenderNode {
                            latest: Some("2.1.0".into()),
                            deprecated: Some("use @foo/core instead".into()),
                            ..leaf("foo", "1.0.0")
                        }],
                        ..leaf("bar", "2.3.1")
                    }],
                    ..leaf("baz", "4.5.6")
                },
                RenderNode {
                    kind: Some("dev"),
                    children: vec![RenderNode {
                        deprecated: Some("use @foo/core instead".into()),
                        deduped: true,
                        ..leaf("foo", "1.0.0")
                    }],
                    ..leaf("qux", "0.9.2")
                },
            ],
        };
        let out = render_tree(&tree);
        let expected = "\
my-project
├─ baz@4.5.6  (latest: 5.2.0)
│  └─ bar@2.3.1  ⛔ blocks: requires foo@~1.0.0, fix needs 2.1.0 → bar update required
│     └─ foo@1.0.0  ⚠ deprecated: use @foo/core instead  (latest: 2.1.0)
└─ qux@0.9.2 (dev)
   └─ foo@1.0.0  ⚠ deprecated: use @foo/core instead (see above)
";
        assert_eq!(out, expected);
    }

    #[test]
    fn renders_circular_marker() {
        console::set_colors_enabled(false);
        let tree = RenderNode {
            children: vec![RenderNode {
                circular: true,
                ..leaf("a", "1.0.0")
            }],
            ..leaf("root", "0.0.0")
        };
        // root carries a version here only for brevity; the marker is what matters.
        let out = render_tree(&tree);
        assert!(out.contains("└─ a@1.0.0  ↻ circular\n"));
    }

    #[test]
    fn renders_combined_replacement_and_blocker_segments() {
        console::set_colors_enabled(false);
        let node = RenderNode {
            deprecated: Some("request has been deprecated".into()),
            fix: Some(
                "no non-deprecated version — needs replacement; blocks: requires uuid@^3.3.2, fix needs 14.0.0 → request update required"
                    .into(),
            ),
            ..leaf("request", "2.88.2")
        };
        // Each segment keeps its own marker: parens for replacement, ⛔ for the blocker.
        assert_eq!(
            label(&node),
            "request@2.88.2  (no non-deprecated version — needs replacement)  ⛔ blocks: requires uuid@^3.3.2, fix needs 14.0.0 → request update required  ⚠ deprecated: request has been deprecated"
        );
    }

    #[test]
    fn renders_empty_deprecation_message_without_colon() {
        console::set_colors_enabled(false);
        let node = RenderNode {
            deprecated: Some(String::new()),
            ..leaf("x", "1.0.0")
        };
        assert_eq!(label(&node), "x@1.0.0  ⚠ deprecated");
    }
}
