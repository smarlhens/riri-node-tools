//! Workspace detection + member iteration for npm, pnpm, and yarn monorepos.

mod detect;
mod members;

pub use detect::{WorkspaceError, WorkspaceProject, detect};
pub use members::WorkspaceMember;
