//! Fuzzy matching for pickers and filters: a case-insensitive subsequence
//! match that prefers what a person means by a short query.
//!
//! A candidate matches when every character of the query appears in it, in
//! order. Among matches, the score rewards the places a person aims at (the
//! start of the candidate, the start of a word, the character after a path
//! separator, a camelCase hump) and runs of consecutive characters, and
//! charges for gaps. The scorer is deterministic and stable: equal scores
//! keep the caller's order, so a list does not shuffle as you type.
//!
//! Matching is greedy and allocation-free apart from the returned positions:
//! a forward scan finds where the earliest full match ends, and a backward
//! scan from there tightens it to the most compact window. That is the
//! matcher the Codewhale engine uses for `@`-mention and slash-command
//! completion (`mention_completion.rs`, `widgets/mod.rs`
//! `fuzzy_chars_in_order`, `file_search.rs` `fuzzy_score` at `58b1dd3dd`),
//! with the engine's density-and-coverage score replaced by per-character
//! bonuses so the highlight and the rank agree.
//!
//! Positions are `char` indices into the candidate as given, so a picker
//! highlights exactly what it paints. Run caller text through
//! [`crate::text::display_safe`] before scoring when the same text is
//! painted, so the indices line up.

use std::cmp::Reverse;

/// A successful match: how good, and which characters of the candidate the
/// query landed on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FuzzyMatch {
    /// Higher is better. Comparable only between candidates of one query.
    pub score: i32,
    /// `char` indices into the candidate, one per query character
    /// (whitespace in the query is ignored), ascending. Empty for an empty
    /// query.
    pub positions: Vec<usize>,
}

/// One ranked candidate from [`rank_matches`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuzzyHit {
    /// Index of the candidate in the order it was given.
    pub index: usize,
    pub score: i32,
    pub positions: Vec<usize>,
}

/// Every matched character.
const SCORE_MATCH: i32 = 16;
/// The first character of the candidate.
const BONUS_START: i32 = 10;
/// The character after a path separator.
const BONUS_PATH: i32 = 10;
/// The character after a space, `_`, `-`, `.` or other punctuation.
const BONUS_WORD: i32 = 8;
/// An uppercase letter after a lowercase one (`camelCase`).
const BONUS_CAMEL: i32 = 7;
/// A digit after a letter (`v2`).
const BONUS_DIGIT: i32 = 3;
/// A character right after the previous match.
const BONUS_RUN: i32 = 5;
/// Typed in the same case as the candidate.
const BONUS_CASE: i32 = 1;
/// The query is the whole candidate.
const BONUS_WHOLE: i32 = 16;
/// Opening a gap between two matches, then each skipped character.
const GAP_OPEN: i32 = 3;
const GAP_EXTEND: i32 = 1;
/// Characters skipped before the first match, up to a cap.
const LEAD_CAP: i32 = 6;
/// Unmatched characters in the candidate, up to a cap, at half a point each,
/// so of two equal matches the shorter wins.
const LENGTH_CAP: usize = 64;

fn fold(c: char) -> char {
    if c.is_ascii() {
        c.to_ascii_lowercase()
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

/// What kind of boundary `c` sits on, given the character before it.
fn boundary(prev: Option<char>, c: char) -> i32 {
    let Some(prev) = prev else {
        return BONUS_START;
    };
    if prev == '/' || prev == '\\' {
        BONUS_PATH
    } else if !prev.is_alphanumeric() {
        BONUS_WORD
    } else if prev.is_lowercase() && c.is_uppercase() {
        BONUS_CAMEL
    } else if prev.is_alphabetic() && c.is_ascii_digit() {
        BONUS_DIGIT
    } else {
        0
    }
}

/// The query's characters, folded, whitespace dropped.
fn query_chars(query: &str) -> impl DoubleEndedIterator<Item = char> + Clone + '_ {
    query.chars().filter(|c| !c.is_whitespace()).map(fold)
}

/// Match `query` against `candidate`, or `None` when the query is not a
/// subsequence of it. An empty query matches everything with a zero score.
#[must_use]
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<FuzzyMatch> {
    let wanted = query_chars(query).count();
    if wanted == 0 {
        return Some(FuzzyMatch::default());
    }

    // Forward: the earliest end of a full match.
    let mut need = query_chars(query);
    let mut next = need.next();
    let mut end = None;
    for (i, c) in candidate.chars().enumerate() {
        if Some(fold(c)) == next {
            next = need.next();
            if next.is_none() {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;
    let len = candidate.chars().count();

    // Backward: the latest start that still reaches `end`.
    let mut need = query_chars(query).rev();
    let mut next = need.next();
    let mut positions = Vec::with_capacity(wanted);
    for (i, c) in candidate.chars().rev().enumerate() {
        let at = len - 1 - i;
        if at > end {
            continue;
        }
        if Some(fold(c)) == next {
            positions.push(at);
            next = need.next();
            if next.is_none() {
                break;
            }
        }
    }
    positions.reverse();
    debug_assert_eq!(positions.len(), wanted);

    let score = score_positions(query, candidate, &positions, len);
    Some(FuzzyMatch { score, positions })
}

fn score_positions(query: &str, candidate: &str, positions: &[usize], len: usize) -> i32 {
    let mut typed = query.chars().filter(|c| !c.is_whitespace());
    let mut score = 0;
    let mut prev: Option<char> = None;
    let mut last: Option<usize> = None;
    let mut run_boundary = 0;
    let mut k = 0;
    for (i, c) in candidate.chars().enumerate() {
        if k < positions.len() && positions[k] == i {
            let b = boundary(prev, c);
            score += SCORE_MATCH;
            match last {
                Some(l) if l + 1 == i => score += BONUS_RUN.max(run_boundary).max(b),
                Some(l) => {
                    let gap = i32::try_from(i - l - 1).unwrap_or(i32::MAX);
                    score -= GAP_OPEN + (gap - 1).max(0) * GAP_EXTEND;
                    score += b;
                    run_boundary = b;
                }
                None => {
                    score -= i32::try_from(i).unwrap_or(i32::MAX).min(LEAD_CAP) * GAP_EXTEND;
                    score += b;
                    run_boundary = b;
                }
            }
            if typed.next() == Some(c) {
                score += BONUS_CASE;
            }
            last = Some(i);
            k += 1;
        }
        prev = Some(c);
    }
    if positions.len() == len {
        score += BONUS_WHOLE;
    }
    let unmatched = (len - positions.len()).min(LENGTH_CAP);
    score - i32::try_from(unmatched / 2).unwrap_or(0)
}

/// Rank `items` against `query`: the matching ones with their scores and
/// positions, best first; equal scores keep their original order. An empty
/// query returns every item in its original order with no positions.
pub fn rank_matches<S: AsRef<str>>(
    query: &str,
    items: impl IntoIterator<Item = S>,
) -> Vec<FuzzyHit> {
    let mut hits: Vec<FuzzyHit> = items
        .into_iter()
        .enumerate()
        .filter_map(|(index, item)| {
            fuzzy_score(query, item.as_ref()).map(|m| FuzzyHit {
                index,
                score: m.score,
                positions: m.positions,
            })
        })
        .collect();
    hits.sort_by_key(|h| (Reverse(h.score), h.index));
    hits
}

/// The indices of the items that match `query`, best first, equal scores in
/// their original order. An empty query returns every index in order.
pub fn rank<S: AsRef<str>>(query: &str, items: impl IntoIterator<Item = S>) -> Vec<usize> {
    rank_matches(query, items)
        .into_iter()
        .map(|h| h.index)
        .collect()
}
