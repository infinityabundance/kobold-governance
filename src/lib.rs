//! # kobold-governance
//!
//! Schema and migration governance: copybook version lineage, field-drift detection, layout-compatibility reports, schema-migration contracts, dataset custody chain, and change-impact analysis.
//!
//! Part of the KOBOLD ecosystem -- independently-authored forensic tooling. Apache-2.0. This crate
//! contains no GnuCOBOL/libcob source; any interaction with COBOL semantics goes through the separate
//! gnucobol-rs crate.
//!
//! Architecture: kobold-* MAY depend on gnucobol-rs; gnucobol-rs MUST NOT depend on kobold-*.
//!
//! Status: SCAFFOLD. Implementation extracted from the gnucobol-rs lab tooling + lineage engine later.
#![forbid(unsafe_code)]

/// Crate scaffold marker; replace with the real public API as the implementation lands.
pub const KOBOLD_CRATE: &str = "kobold-governance";
