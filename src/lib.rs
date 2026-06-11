//! # kobold-governance
//!
//! Copybook version lineage and schema-migration governance: the audit-signoff layer that answers
//! one question -- *did this copybook change break the on-disk record layout?* A COBOL copybook is a
//! physical contract: downstream readers index a field by its byte **offset** and **length**, so a
//! seemingly innocent edit (renumbering a level, widening a `PIC`, reordering two `05`s) silently
//! shifts every following byte and corrupts every consumer that was not recompiled against it.
//!
//! kobold-governance models a copybook as an ordered list of physical fields and [`diff`]s two
//! versions to produce a [`DriftReport`]: a per-field classification (added / removed / resized /
//! retyped / moved / reordered / unchanged) plus drift counts. [`compatibility`] then collapses that
//! report into a single forensic verdict -- [`Compatibility::Identical`],
//! [`Compatibility::CompatibleExtension`] (only new fields appended at the tail), or
//! [`Compatibility::BreakingChange`] (any existing field moved, resized, retyped, or removed).
//!
//! The verdict is deliberately **fail-closed**: anything that cannot be proven layout-preserving is a
//! `BreakingChange`. A clean signoff means the new copybook can read records written under the old
//! one; it never overstates compatibility.
//!
//! Part of the KOBOLD ecosystem -- independently-authored forensic tooling, Apache-2.0. This crate
//! contains **no GnuCOBOL/libcob source** and depends only on `serde`/`serde_json`; the copybook
//! layout conventions it models (level numbers, byte offsets, `PIC`/`USAGE` storage) are public,
//! long-documented data-description conventions.
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// The physical storage class of a field -- what determines how many bytes it occupies and how those
/// bytes are interpreted. Two fields with the same byte length but different `FieldKind` are *not*
/// layout-compatible: a reader decoding packed decimal cannot read display digits from the same span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum FieldKind {
    /// `PIC X(n)` -- opaque alphanumeric text.
    Alphanumeric,
    /// `PIC 9(d)V9(s) [DISPLAY]` -- zoned/display numeric (one byte per digit).
    Display { digits: u32, scale: u32, signed: bool },
    /// `PIC S9(d)V9(s) COMP-3` -- packed decimal (BCD, `d/2 + 1` bytes).
    Packed { digits: u32, scale: u32, signed: bool },
    /// `PIC S9(d) COMP/COMP-4/COMP-5` -- binary integer of a fixed byte width.
    Binary { bytes: u32, signed: bool },
    /// A group item (`01`/`05` with subordinate fields) -- a container, not a leaf storage cell.
    Group,
}

/// One field within a copybook layout: its name, level number, 0-based byte offset within the record,
/// physical byte length, and storage [`FieldKind`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    /// COBOL level number (`01`, `05`, `10`, ...).
    pub level: u16,
    /// 0-based byte offset of the field within the record.
    pub offset: usize,
    /// Physical byte length the field occupies on disk.
    pub len: usize,
    pub kind: FieldKind,
}

/// A named, versioned copybook: an ordered list of physical fields. Order is significant -- it is the
/// on-disk field order, and reordering two fields is itself a layout change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copybook {
    pub name: String,
    /// A free-form version label (`v1`, `2024-03`, a git sha, ...).
    pub version: String,
    #[serde(default)]
    pub fields: Vec<Field>,
}

/// How a single field changed between two copybook versions. Fields are matched **by name**; an
/// unmatched name on either side is [`ChangeKind::Added`] or [`ChangeKind::Removed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ChangeKind {
    /// Present in the new copybook, absent in the old.
    Added,
    /// Present in the old copybook, absent in the new.
    Removed,
    /// Byte length changed (widened/narrowed) -- shifts every following field.
    Resized,
    /// Storage [`FieldKind`] changed -- same span, different decode.
    Retyped,
    /// Byte offset changed while length and kind held -- the field moved on disk.
    Moved,
    /// Same offset/len/kind, but its ordinal position among the fields changed.
    Reordered,
    /// Identical name, level, offset, length, kind, and position.
    Unchanged,
}

/// A located, classified per-field change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: String,
    pub change: ChangeKind,
    /// Field's offset in the old copybook, when present there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_offset: Option<usize>,
    /// Field's offset in the new copybook, when present there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_offset: Option<usize>,
    /// Human-readable account of the change.
    pub detail: String,
}

/// Per-change drift counts, keyed by [`ChangeKind`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftCounts {
    pub added: usize,
    pub removed: usize,
    pub resized: usize,
    pub retyped: usize,
    pub moved: usize,
    pub reordered: usize,
    pub unchanged: usize,
}

/// The aggregate drift between two copybook versions: the ordered per-field changes plus counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriftReport {
    pub old_name: String,
    pub old_version: String,
    pub new_name: String,
    pub new_version: String,
    pub changes: Vec<FieldChange>,
    pub counts: DriftCounts,
}

impl DriftReport {
    /// `true` when every field is [`ChangeKind::Unchanged`].
    pub fn is_identical(&self) -> bool {
        self.changes.iter().all(|c| c.change == ChangeKind::Unchanged)
    }
}

/// The layout-compatibility verdict between an old and a new copybook. This is the key forensic call:
/// can a reader built against the **new** copybook still read records written under the **old** one?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Compatibility {
    /// The two layouts are byte-for-byte identical.
    Identical,
    /// Only additions, all at the tail; no pre-existing field moved, resized, retyped, removed, or
    /// reordered. Old records remain readable -- the new fields simply read past the old record end.
    CompatibleExtension,
    /// At least one pre-existing field was moved, resized, retyped, removed, or reordered (or an
    /// addition landed before the tail). On-disk layout is broken for existing readers.
    BreakingChange,
}

/// Build a lookup from field name to (ordinal index, field).
fn index_by_name(fields: &[Field]) -> Vec<(&str, usize, &Field)> {
    fields
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.as_str(), i, f))
        .collect()
}

fn find<'a>(idx: &'a [(&str, usize, &'a Field)], name: &str) -> Option<(usize, &'a Field)> {
    idx.iter().find(|(n, _, _)| *n == name).map(|(_, i, f)| (*i, *f))
}

/// Diff two copybook versions and classify every field's drift.
///
/// Matching is **by name**. For each name present in both, the change is determined in priority order
/// (most severe first): `Removed`/`Added` for unmatched names, then `Retyped`, `Resized`, `Moved`,
/// `Reordered`, and finally `Unchanged`. Fail-closed: any structural change to an existing field is
/// surfaced rather than smoothed over.
pub fn diff(old: &Copybook, new: &Copybook) -> DriftReport {
    let old_idx = index_by_name(&old.fields);
    let new_idx = index_by_name(&new.fields);
    let mut changes = Vec::new();
    let mut counts = DriftCounts::default();

    // Walk the new copybook in order: matched fields classify by structural change; new names are Added.
    for (new_pos, nf) in new.fields.iter().enumerate() {
        match find(&old_idx, &nf.name) {
            None => {
                counts.added += 1;
                changes.push(FieldChange {
                    field: nf.name.clone(),
                    change: ChangeKind::Added,
                    old_offset: None,
                    new_offset: Some(nf.offset),
                    detail: format!(
                        "field added (level {}, offset {}, len {})",
                        nf.level, nf.offset, nf.len
                    ),
                });
            }
            Some((old_pos, of)) => {
                let change = classify(of, old_pos, nf, new_pos);
                bump(&mut counts, change);
                changes.push(FieldChange {
                    field: nf.name.clone(),
                    change,
                    old_offset: Some(of.offset),
                    new_offset: Some(nf.offset),
                    detail: describe(of, nf, change),
                });
            }
        }
    }

    // Names present in old but absent in new are Removed.
    for of in &old.fields {
        if find(&new_idx, &of.name).is_none() {
            counts.removed += 1;
            changes.push(FieldChange {
                field: of.name.clone(),
                change: ChangeKind::Removed,
                old_offset: Some(of.offset),
                new_offset: None,
                detail: format!(
                    "field removed (was level {}, offset {}, len {})",
                    of.level, of.offset, of.len
                ),
            });
        }
    }

    DriftReport {
        old_name: old.name.clone(),
        old_version: old.version.clone(),
        new_name: new.name.clone(),
        new_version: new.version.clone(),
        changes,
        counts,
    }
}

/// Classify a matched field. Most-severe-wins: a field that is both retyped and resized reports
/// `Retyped`, the more fundamental break.
fn classify(of: &Field, old_pos: usize, nf: &Field, new_pos: usize) -> ChangeKind {
    if of.kind != nf.kind {
        ChangeKind::Retyped
    } else if of.len != nf.len {
        ChangeKind::Resized
    } else if of.offset != nf.offset {
        ChangeKind::Moved
    } else if old_pos != new_pos {
        ChangeKind::Reordered
    } else {
        ChangeKind::Unchanged
    }
}

fn describe(of: &Field, nf: &Field, change: ChangeKind) -> String {
    match change {
        ChangeKind::Retyped => format!("kind changed {:?} -> {:?}", of.kind, nf.kind),
        ChangeKind::Resized => format!("byte length {} -> {} (shifts following fields)", of.len, nf.len),
        ChangeKind::Moved => format!("offset {} -> {} (field moved on disk)", of.offset, nf.offset),
        ChangeKind::Reordered => "field reordered among siblings (offset/len/kind held)".to_string(),
        ChangeKind::Unchanged => "no change".to_string(),
        ChangeKind::Added | ChangeKind::Removed => unreachable!("matched fields are not added/removed"),
    }
}

fn bump(counts: &mut DriftCounts, change: ChangeKind) {
    match change {
        ChangeKind::Added => counts.added += 1,
        ChangeKind::Removed => counts.removed += 1,
        ChangeKind::Resized => counts.resized += 1,
        ChangeKind::Retyped => counts.retyped += 1,
        ChangeKind::Moved => counts.moved += 1,
        ChangeKind::Reordered => counts.reordered += 1,
        ChangeKind::Unchanged => counts.unchanged += 1,
    }
}

/// Collapse a [`DriftReport`] into a single [`Compatibility`] verdict.
///
/// - **Identical**: every field unchanged.
/// - **CompatibleExtension**: the only changes are `Added` fields, and every added field sits *after*
///   all retained fields in the new layout (a true tail append). No retained field moved, resized,
///   retyped, was removed, or reordered.
/// - **BreakingChange**: anything else. Fail-closed -- a removal, retype, resize, move, reorder, or an
///   insertion that is not strictly at the tail breaks the on-disk layout for existing readers.
pub fn compatibility(report: &DriftReport) -> Compatibility {
    let c = &report.counts;

    // Any change to a pre-existing field is, by definition, a break.
    if c.removed > 0 || c.resized > 0 || c.retyped > 0 || c.moved > 0 || c.reordered > 0 {
        return Compatibility::BreakingChange;
    }

    if c.added == 0 {
        // No additions and (per the guard above) no structural changes -> identical layout.
        return Compatibility::Identical;
    }

    // Additions only: a compatible extension iff every Added field appears after the last retained
    // (Unchanged) field in the report's ordering -- i.e. the additions form a contiguous tail. The
    // changes are emitted in new-copybook on-disk order, so find the last Unchanged position and
    // require every Added field to follow it. Removed entries cannot appear here (returned above).
    let last_retained = report
        .changes
        .iter()
        .enumerate()
        .filter(|(_, ch)| ch.change == ChangeKind::Unchanged)
        .map(|(i, _)| i)
        .next_back();

    let additions_all_at_tail = match last_retained {
        // No retained field at all -- a wholly new layout is treated conservatively as breaking,
        // since there is no shared anchor proving old records remain readable.
        None => false,
        Some(last) => report
            .changes
            .iter()
            .enumerate()
            .filter(|(_, ch)| ch.change == ChangeKind::Added)
            .all(|(i, _)| i > last),
    };

    if additions_all_at_tail {
        Compatibility::CompatibleExtension
    } else {
        Compatibility::BreakingChange
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fld(name: &str, level: u16, offset: usize, len: usize, kind: FieldKind) -> Field {
        Field { name: name.into(), level, offset, len, kind }
    }

    fn base() -> Copybook {
        Copybook {
            name: "ACCT-REC".into(),
            version: "v1".into(),
            fields: vec![
                fld("ACCT-ID", 5, 0, 8, FieldKind::Display { digits: 8, scale: 0, signed: false }),
                fld("ACCT-NAME", 5, 8, 20, FieldKind::Alphanumeric),
                fld("ACCT-BAL", 5, 28, 5, FieldKind::Packed { digits: 9, scale: 2, signed: true }),
            ],
        }
    }

    #[test]
    fn identical_copybooks_are_identical() {
        let report = diff(&base(), &base());
        assert!(report.is_identical(), "{:?}", report.changes);
        assert_eq!(report.counts.unchanged, 3);
        assert_eq!(compatibility(&report), Compatibility::Identical);
    }

    #[test]
    fn tail_appended_field_is_compatible_extension() {
        let old = base();
        let mut new = base();
        new.version = "v2".into();
        new.fields.push(fld(
            "ACCT-OPENED",
            5,
            33,
            8,
            FieldKind::Display { digits: 8, scale: 0, signed: false },
        ));
        let report = diff(&old, &new);
        assert_eq!(report.counts.added, 1);
        assert_eq!(report.counts.unchanged, 3);
        assert_eq!(compatibility(&report), Compatibility::CompatibleExtension);
    }

    #[test]
    fn resized_field_is_breaking() {
        let old = base();
        let mut new = base();
        new.version = "v2".into();
        // widen ACCT-NAME from 20 to 30 -> shifts ACCT-BAL.
        new.fields[1].len = 30;
        new.fields[2].offset = 38;
        let report = diff(&old, &new);
        assert!(report.changes.iter().any(|c| c.field == "ACCT-NAME" && c.change == ChangeKind::Resized));
        assert!(report.changes.iter().any(|c| c.field == "ACCT-BAL" && c.change == ChangeKind::Moved));
        assert_eq!(compatibility(&report), Compatibility::BreakingChange);
    }

    #[test]
    fn moved_field_is_breaking() {
        let old = base();
        let mut new = base();
        new.fields[2].offset = 27; // ACCT-BAL slid back one byte
        let report = diff(&old, &new);
        assert!(report.changes.iter().any(|c| c.field == "ACCT-BAL" && c.change == ChangeKind::Moved));
        assert_eq!(compatibility(&report), Compatibility::BreakingChange);
    }

    #[test]
    fn retyped_field_is_breaking() {
        let old = base();
        let mut new = base();
        // same offset/len, but decode the balance as binary instead of packed.
        new.fields[2].kind = FieldKind::Binary { bytes: 5, signed: true };
        let report = diff(&old, &new);
        let bal = report.changes.iter().find(|c| c.field == "ACCT-BAL").unwrap();
        assert_eq!(bal.change, ChangeKind::Retyped);
        assert_eq!(compatibility(&report), Compatibility::BreakingChange);
    }

    #[test]
    fn removed_field_is_breaking() {
        let old = base();
        let mut new = base();
        new.fields.remove(1); // drop ACCT-NAME
        let report = diff(&old, &new);
        assert!(report.changes.iter().any(|c| c.field == "ACCT-NAME" && c.change == ChangeKind::Removed));
        assert_eq!(report.counts.removed, 1);
        assert_eq!(compatibility(&report), Compatibility::BreakingChange);
    }

    #[test]
    fn insertion_before_tail_is_breaking() {
        let old = base();
        let mut new = base();
        // insert a new field in the MIDDLE -- not a tail append; old offsets after it shift.
        new.fields.insert(1, fld("ACCT-TYPE", 5, 8, 2, FieldKind::Alphanumeric));
        new.fields[2].offset = 10; // ACCT-NAME shifted
        new.fields[3].offset = 30; // ACCT-BAL shifted
        let report = diff(&old, &new);
        // ACCT-TYPE is Added, but ACCT-NAME/ACCT-BAL moved -> breaking.
        assert_eq!(compatibility(&report), Compatibility::BreakingChange);
    }

    #[test]
    fn pure_added_without_retained_anchor_is_breaking() {
        // A new copybook that shares no field names with the old one cannot be proven readable.
        let old = base();
        let new = Copybook {
            name: "ACCT-REC".into(),
            version: "v2".into(),
            fields: vec![fld("WHOLLY-NEW", 1, 0, 4, FieldKind::Alphanumeric)],
        };
        let report = diff(&old, &new);
        assert_eq!(compatibility(&report), Compatibility::BreakingChange);
    }

    #[test]
    fn reordered_field_is_breaking() {
        // Two identical-shape fields whose on-disk order swaps -> Reordered -> breaking.
        let a = fld("F1", 5, 0, 4, FieldKind::Alphanumeric);
        let b = fld("F2", 5, 0, 4, FieldKind::Alphanumeric);
        let old = Copybook { name: "R".into(), version: "v1".into(), fields: vec![a.clone(), b.clone()] };
        let new = Copybook { name: "R".into(), version: "v2".into(), fields: vec![b, a] };
        let report = diff(&old, &new);
        assert!(report.changes.iter().any(|c| c.change == ChangeKind::Reordered));
        assert_eq!(compatibility(&report), Compatibility::BreakingChange);
    }

    #[test]
    fn report_round_trips_through_json() {
        let report = diff(&base(), &base());
        let s = serde_json::to_string(&report).unwrap();
        let back: DriftReport = serde_json::from_str(&s).unwrap();
        assert_eq!(report, back);
    }
}
