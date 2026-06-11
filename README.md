# kobold-governance

Copybook schema-migration governance: diff two copybook layout versions, classify per-field drift, and
emit a fail-closed on-disk-layout compatibility verdict.

A COBOL copybook is a physical contract: downstream readers index a field by its byte **offset** and
**length**. A seemingly innocent edit -- widening a `PIC`, reordering two `05`s, retyping a `COMP-3`
field as binary -- silently shifts every following byte and corrupts every consumer that was not
recompiled. kobold-governance is the audit-signoff layer that answers one question: *did this copybook
change break the on-disk record layout?*

## What it does

- Models a copybook as an ordered list of physical fields (`name`, `level`, `offset`, `len`, `kind`).
- `diff(old, new)` classifies every field, matched by name, into a `DriftReport`:
  `Added` / `Removed` / `Resized` / `Retyped` / `Moved` / `Reordered` / `Unchanged`, with drift counts.
- `compatibility(&report)` collapses the report into a single verdict:
  - `Identical` -- byte-for-byte the same layout.
  - `CompatibleExtension` -- only new fields, all appended at the tail; old records still read.
  - `BreakingChange` -- any existing field moved, resized, retyped, removed, or reordered.

The verdict is **fail-closed**: anything that cannot be proven layout-preserving is a `BreakingChange`.
A clean signoff means the new copybook can read records written under the old one; it never overstates
compatibility.

## CLI

```text
kobold-governance diff <old.json> <new.json> [--pretty]
```

Reads two `Copybook` JSON documents, prints the drift report and verdict as JSON, and exits non-zero on
a `BreakingChange`. Example copybook JSON:

```json
{"name":"ACCT-REC","version":"v1","fields":[
  {"name":"ACCT-ID","level":5,"offset":0,"len":8,"kind":{"display":{"digits":8,"scale":0,"signed":false}}},
  {"name":"ACCT-BAL","level":5,"offset":8,"len":5,"kind":{"packed":{"digits":9,"scale":2,"signed":true}}}]}
```

## KOBOLD ecosystem

Part of KOBOLD -- a forensic archaeology and evidence system for legacy COBOL estates. Independently
authored; contains **no GnuCOBOL source** and depends only on `serde`/`serde_json`. The copybook layout
conventions it models (level numbers, byte offsets, `PIC`/`USAGE` storage) are public, long-documented
data-description conventions.

- gnucobol-rs (separate crate) = the oracle-proven semantic primitive layer.
- kobold-* = the forensic-intelligence layer.
- kobold-* MAY depend on gnucobol-rs; gnucobol-rs MUST NOT depend on kobold-*.

## License

Apache-2.0 (see LICENSE).
