//! A listr2-like task runner for CLI tools.
//!
//! Provides animated spinners, verbose logging, or silent execution
//! depending on the chosen renderer mode. Shared by both NCE and NPD.

use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;

/// Renderer mode — mirrors listr2's renderer selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererMode {
    /// Animated spinners (default interactive mode).
    Default,
    /// Log-style output with timestamps (verbose/debug).
    Verbose,
    /// Simple line-by-line output (non-interactive terminals).
    Simple,
    /// No output at all.
    Silent,
}

/// A task runner that manages multiple concurrent tasks with spinners.
pub struct TaskRunner {
    multi: MultiProgress,
    mode: RendererMode,
}

impl TaskRunner {
    /// Create a new task runner with the given renderer mode.
    #[must_use]
    pub fn new(mode: RendererMode) -> Self {
        let multi = if mode == RendererMode::Silent {
            let mp = MultiProgress::new();
            mp.set_draw_target(indicatif::ProgressDrawTarget::hidden());
            mp
        } else {
            MultiProgress::new()
        };

        Self { multi, mode }
    }

    /// Start a new task with the given title.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn task(&self, title: &str) -> TaskHandle {
        match self.mode {
            RendererMode::Default => {
                let pb = self.multi.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::with_template("  {spinner:.cyan} {msg}")
                        .expect("valid template"),
                );
                pb.set_message(title.to_string());
                pb.enable_steady_tick(Duration::from_millis(80));
                TaskHandle {
                    progress_bar: Some(pb),
                    mode: self.mode,
                }
            }
            RendererMode::Verbose => {
                eprintln!("  {} {title}...", style("▸").dim());
                TaskHandle {
                    progress_bar: None,
                    mode: self.mode,
                }
            }
            RendererMode::Simple => {
                eprintln!("  {title}");
                TaskHandle {
                    progress_bar: None,
                    mode: self.mode,
                }
            }
            RendererMode::Silent => TaskHandle {
                progress_bar: None,
                mode: self.mode,
            },
        }
    }
}

/// Handle to a running task — use to complete, skip, or fail it.
pub struct TaskHandle {
    progress_bar: Option<ProgressBar>,
    mode: RendererMode,
}

impl TaskHandle {
    /// Mark the task as successfully completed.
    #[allow(clippy::missing_panics_doc)]
    pub fn complete(self, title: &str) {
        match self.mode {
            RendererMode::Default => {
                if let Some(pb) = &self.progress_bar {
                    pb.set_style(ProgressStyle::with_template("  {msg}").expect("valid template"));
                    pb.finish_with_message(format!("{} {title}", style("✓").green()));
                }
            }
            RendererMode::Verbose => {
                eprintln!("  {} {title}", style("✓").green());
            }
            RendererMode::Simple => {
                eprintln!("  {title}");
            }
            RendererMode::Silent => {}
        }
    }

    /// Mark the task as skipped with a reason.
    #[allow(clippy::missing_panics_doc)]
    pub fn skip(self, title: &str, reason: &str) {
        match self.mode {
            RendererMode::Default => {
                if let Some(pb) = &self.progress_bar {
                    pb.set_style(ProgressStyle::with_template("  {msg}").expect("valid template"));
                    pb.finish_with_message(format!(
                        "{} {title} {}",
                        style("⊘").yellow(),
                        style(format!("[skipped: {reason}]")).dim()
                    ));
                }
            }
            RendererMode::Verbose => {
                eprintln!(
                    "  {} {title} {}",
                    style("⊘").yellow(),
                    style(format!("[skipped: {reason}]")).dim()
                );
            }
            RendererMode::Simple => {
                eprintln!("  {title} [skipped: {reason}]");
            }
            RendererMode::Silent => {}
        }
    }

    /// Mark the task as failed.
    #[allow(clippy::missing_panics_doc)]
    pub fn fail(self, title: &str) {
        match self.mode {
            RendererMode::Default => {
                if let Some(pb) = &self.progress_bar {
                    pb.set_style(ProgressStyle::with_template("  {msg}").expect("valid template"));
                    pb.finish_with_message(format!("{} {title}", style("✗").red()));
                }
            }
            RendererMode::Verbose | RendererMode::Simple => {
                eprintln!("  {} {title}", style("✗").red());
            }
            RendererMode::Silent => {}
        }
    }

    /// Update the task title (animated mode only).
    pub fn update_title(&self, title: &str) {
        if let Some(pb) = &self.progress_bar {
            pb.set_message(title.to_string());
        }
    }
}
