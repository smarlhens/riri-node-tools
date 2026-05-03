//! Node.js release lifecycle data and lookups.
//!
//! Exposes types describing every Node.js major (lifecycle dates, status,
//! bundled npm versions) and a single entry point [`LifecycleData::bundled`]
//! that returns the snapshot shipped with this crate.

mod cache;
mod data;
mod major;
mod schema;

pub use cache::CacheError;
pub use data::{LifecycleData, LookupError};
pub use major::{MajorInfo, ReleaseEntry, Status};
pub use schema::{SCHEMA_VERSION, SchemaError};

/// User-selectable lifecycle gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// All statuses, including `EndOfLife` and `Pending`.
    Any,
    /// Drops `Pending` only. Keeps `EndOfLife`.
    Stable,
    /// Drops `Pending` and `EndOfLife`.
    Supported,
    /// Active + Maintenance LTS only.
    Lts,
    /// Maintenance LTS only.
    Maintenance,
}

impl Policy {
    /// Returns the set of statuses allowed by this policy.
    #[must_use]
    pub fn allowed_statuses(self) -> &'static [Status] {
        match self {
            Self::Any => &[
                Status::Pending,
                Status::Current,
                Status::Active,
                Status::Maintenance,
                Status::EndOfLife,
            ],
            Self::Stable => &[
                Status::Current,
                Status::Active,
                Status::Maintenance,
                Status::EndOfLife,
            ],
            Self::Supported => &[Status::Current, Status::Active, Status::Maintenance],
            Self::Lts => &[Status::Active, Status::Maintenance],
            Self::Maintenance => &[Status::Maintenance],
        }
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn any_allows_all_statuses() {
        let allowed = Policy::Any.allowed_statuses();
        assert!(allowed.contains(&Status::Pending));
        assert!(allowed.contains(&Status::Current));
        assert!(allowed.contains(&Status::Active));
        assert!(allowed.contains(&Status::Maintenance));
        assert!(allowed.contains(&Status::EndOfLife));
    }

    #[test]
    fn stable_drops_pending_only() {
        let allowed = Policy::Stable.allowed_statuses();
        assert!(!allowed.contains(&Status::Pending));
        assert!(allowed.contains(&Status::EndOfLife));
    }

    #[test]
    fn supported_drops_pending_and_eol() {
        let allowed = Policy::Supported.allowed_statuses();
        assert!(!allowed.contains(&Status::Pending));
        assert!(!allowed.contains(&Status::EndOfLife));
        assert!(allowed.contains(&Status::Current));
    }

    #[test]
    fn lts_keeps_active_and_maintenance_only() {
        let allowed = Policy::Lts.allowed_statuses();
        assert_eq!(allowed.len(), 2);
        assert!(allowed.contains(&Status::Active));
        assert!(allowed.contains(&Status::Maintenance));
    }

    #[test]
    fn maintenance_keeps_maintenance_only() {
        let allowed = Policy::Maintenance.allowed_statuses();
        assert_eq!(allowed, &[Status::Maintenance]);
    }
}
