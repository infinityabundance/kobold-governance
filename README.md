# kobold-governance

Schema and migration governance: copybook version lineage, field-drift detection, layout-compatibility reports, schema-migration contracts, dataset custody chain, and change-impact analysis.

**Part of KOBOLD** -- a forensic archaeology and evidence system for legacy COBOL estates: it maps real COBOL
codebases, generated oracle witnesses, compiler-profile behavior, and migration risk into court-backed
receipts. Independently-authored tooling; contains no GnuCOBOL source.

## Architecture
- gnucobol-rs (separate crate) = the oracle-proven semantic primitive layer.
- kobold-* = the forensic-intelligence layer.
- kobold-* MAY depend on gnucobol-rs; gnucobol-rs MUST NOT depend on kobold-*.

## License
Apache-2.0 (see LICENSE).

## Status
Scaffold only -- local repo initialized, no implementation extracted yet, not pushed, not published.
