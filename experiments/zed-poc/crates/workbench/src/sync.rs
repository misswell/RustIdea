//! Pure three-way merge and coalesced-diff helpers used by the GPUI document
//! mirror. The backend OpenIDEA `Documents` registry remains authoritative;
//! these functions never replace it and only decide whether pending local
//! transactions can be rebased safely or must become an explicit conflict.

use std::collections::BTreeSet;

use ide_core::ChangeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeOutcome {
    Merged(String),
    Conflict,
}

#[derive(Clone, Debug)]
struct BaseChange {
    from: u32,
    to: u32,
    insert: String,
}

/// A coalesced minimal diff from `base` to `local`, suitable for one backend
/// `Documents::edit` call at the confirmed base version.
pub fn coalesced_diff(base: &str, local: &str) -> ChangeSet {
    ide_text::Document::new(base).diff(local)
}

/// Merges local pending edits and an external backend text when their changed
/// ranges do not overlap. Any ambiguity becomes an explicit conflict instead
/// of silently choosing a side.
pub fn merge_three_way(base: &str, local: &str, backend: &str) -> MergeOutcome {
    if local == base {
        return MergeOutcome::Merged(backend.to_owned());
    }
    if backend == base {
        return MergeOutcome::Merged(local.to_owned());
    }
    if local == backend {
        return MergeOutcome::Merged(local.to_owned());
    }

    let local_changes = changes(&coalesced_diff(base, local));
    let backend_changes = changes(&coalesced_diff(base, backend));
    if overlaps(&local_changes, &backend_changes) {
        return MergeOutcome::Conflict;
    }

    let mut all = local_changes;
    all.extend(backend_changes);
    all.sort_by_key(|change| (change.from, change.to));

    let positions: Vec<u32> = all
        .iter()
        .flat_map(|change| [change.from, change.to])
        .collect();
    let offsets = utf16_offsets(base, &positions);
    let mut output = String::with_capacity(base.len().max(local.len()).max(backend.len()));
    let mut cursor = 0usize;
    for change in all {
        let from = offsets[&change.from];
        let to = offsets[&change.to];
        if from < cursor || to < from {
            return MergeOutcome::Conflict;
        }
        output.push_str(&base[cursor..from]);
        output.push_str(&change.insert);
        cursor = to;
    }
    output.push_str(&base[cursor..]);
    MergeOutcome::Merged(output)
}

fn changes(set: &ChangeSet) -> Vec<BaseChange> {
    set.changes
        .iter()
        .filter(|change| change.from != change.to || !change.insert.is_empty())
        .map(|change| BaseChange {
            from: change.from,
            to: change.to,
            insert: change.insert.clone(),
        })
        .collect()
}

fn overlaps(left: &[BaseChange], right: &[BaseChange]) -> bool {
    for a in left {
        for b in right {
            let a_insert = a.from == a.to;
            let b_insert = b.from == b.to;
            if a_insert && b_insert {
                if a.from == b.from {
                    return true;
                }
                continue;
            }
            if a_insert {
                if a.from > b.from && a.from < b.to {
                    return true;
                }
                continue;
            }
            if b_insert {
                if b.from > a.from && b.from < a.to {
                    return true;
                }
                continue;
            }
            if a.from.max(b.from) < a.to.min(b.to) {
                return true;
            }
        }
    }
    false
}

fn utf16_offsets(text: &str, positions: &[u32]) -> std::collections::BTreeMap<u32, usize> {
    let wanted: BTreeSet<u32> = positions.iter().copied().collect();
    let mut result = std::collections::BTreeMap::new();
    if wanted.contains(&0) {
        result.insert(0, 0);
    }
    let mut utf16 = 0u32;
    for (byte, character) in text.char_indices() {
        utf16 += character.len_utf16() as u32;
        if wanted.contains(&utf16) {
            result.insert(utf16, byte + character.len_utf8());
        }
    }
    for position in wanted {
        result.entry(position).or_insert_with(|| {
            if position as usize >= text.encode_utf16().count() {
                text.len()
            } else {
                // A position inside a surrogate pair is not a valid change
                // boundary; callers reject it as a conflict.
                0
            }
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesced_diff_is_lossless_for_many_sequential_inputs() {
        let base = "start\n";
        let mut local = String::from(base);
        for _ in 0..10_000 {
            local.push('x');
        }
        let diff = coalesced_diff(base, &local);
        let mut backend = ide_text::Document::new(base);
        backend.apply(0, &diff).unwrap();
        assert_eq!(backend.text(), local);
    }

    #[test]
    fn disjoint_insertions_merge() {
        let base = "abcdef";
        let outcome = merge_three_way(base, "abXcdef", "abcdYef");
        assert_eq!(outcome, MergeOutcome::Merged("abXcdYef".into()));
    }

    #[test]
    fn deletion_and_later_insertion_merge() {
        let base = "abcdef";
        let outcome = merge_three_way(base, "abef", "abcdXef");
        assert_eq!(outcome, MergeOutcome::Merged("abXef".into()));
    }

    #[test]
    fn overlapping_edits_conflict_without_losing_local_text() {
        let base = "abcdef";
        let outcome = merge_three_way(base, "abXXef", "abcZZef");
        assert_eq!(outcome, MergeOutcome::Conflict);
    }

    #[test]
    fn same_point_insertions_conflict() {
        let base = "abc";
        let outcome = merge_three_way(base, "abXc", "abYc");
        assert_eq!(outcome, MergeOutcome::Conflict);
    }

    #[test]
    fn backend_or_local_unchanged_is_a_direct_take() {
        assert_eq!(
            merge_three_way("abc", "abXc", "abc"),
            MergeOutcome::Merged("abXc".into())
        );
        assert_eq!(
            merge_three_way("abc", "abc", "abYc"),
            MergeOutcome::Merged("abYc".into())
        );
    }

    #[test]
    fn utf16_offsets_handle_astral_characters() {
        let text = "a😀bc";
        let positions = [0, 1, 3, 5];
        let offsets = utf16_offsets(text, &positions);
        assert_eq!(offsets.get(&0), Some(&0));
        assert_eq!(offsets.get(&1), Some(&1));
        assert_eq!(offsets.get(&3), Some(&5));
        assert_eq!(offsets.get(&5), Some(&7));
    }

    #[test]
    fn non_overlapping_utf16_ranges_merge_around_astral_text() {
        let base = "a😀bcdef";
        let outcome = merge_three_way(base, "a😀bXcdef", "a😀bcYdef");
        assert_eq!(outcome, MergeOutcome::Merged("a😀bXcYdef".into()));
    }
}
