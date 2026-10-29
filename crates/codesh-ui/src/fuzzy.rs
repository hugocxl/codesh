//! Fuzzy matching and ranking shared by every list that filters as the user types.
//!
//! Match offsets are UTF-8 byte ranges into the candidate, always on character
//! boundaries, so they can highlight the candidate text directly.

use std::ops::Range;

use gpui::{FontWeight, HighlightStyle, Hsla, SharedString, StyledText};

/// A candidate that contains every query character in order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Match {
    pub score: i64,
    pub ranges: Vec<Range<usize>>,
}

/// A candidate kept by [`rank`], with its score and matched ranges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ranked<T> {
    pub item: T,
    pub score: i64,
    pub ranges: Vec<Range<usize>>,
}

/// Lowercases a query into the characters [`fuzzy`] compares against.
pub fn normalize(query: &str) -> Vec<char> {
    query
        .chars()
        .map(|ch| ch.to_lowercase().next().unwrap_or(ch))
        .collect()
}

/// Scores `candidate` against a [`normalize`]d query.
///
/// Every possible first character is tried so an early directory match cannot
/// hide a stronger consecutive match in the basename. Matches after the last
/// `/`, at word or camel-case boundaries, and consecutive matches score higher.
pub fn fuzzy(candidate: &str, query: &[char]) -> Option<Match> {
    if query.is_empty() {
        return Some(Match::default());
    }
    let chars: Vec<_> = candidate.char_indices().collect();
    let basename = candidate.rfind('/').map_or(0, |index| index + 1);
    let mut best: Option<Match> = None;
    for start in 0..chars.len() {
        let mut matched = 0;
        let mut score = 0;
        let mut ranges = Vec::new();
        let mut previous = None;
        for (index, &(offset, ch)) in chars.iter().enumerate().skip(start) {
            if ch.to_lowercase().next() == Some(query[matched]) {
                score += 10 + if offset >= basename { 20 } else { 0 };
                if index == 0
                    || !chars[index - 1].1.is_alphanumeric()
                    || (chars[index - 1].1.is_lowercase() && ch.is_uppercase())
                {
                    score += 15;
                }
                if previous == Some(index.saturating_sub(1)) {
                    score += 25;
                }
                previous = Some(index);
                ranges.push(offset..offset + ch.len_utf8());
                matched += 1;
                if matched == query.len() {
                    if best.as_ref().is_none_or(|old| score > old.score) {
                        best = Some(Match { score, ranges });
                    }
                    break;
                }
            } else if index == start {
                break;
            }
        }
    }
    best
}

/// The best `limit` candidates for `query`, highest score first. Ties go to the
/// shorter key, then to the lexicographically smaller one.
pub fn rank<T>(
    candidates: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> &str,
    query: &str,
    limit: usize,
) -> Vec<Ranked<T>> {
    let query = normalize(query);
    let mut ranked: Vec<Ranked<T>> = Vec::with_capacity(limit.saturating_add(1).min(1024));
    if limit == 0 {
        return ranked;
    }
    for item in candidates {
        let text = key(&item);
        let Some(Match { score, ranges }) = fuzzy(text, &query) else {
            continue;
        };
        let position = ranked.partition_point(|existing| {
            let existing_key = key(&existing.item);
            existing.score > score
                || (existing.score == score
                    && (existing_key.len(), existing_key) <= (text.len(), text))
        });
        if position < limit {
            ranked.insert(
                position,
                Ranked {
                    item,
                    score,
                    ranges,
                },
            );
            ranked.truncate(limit);
        }
    }
    ranked
}

/// `text` with `ranges` drawn bold in `color`. Ranges that do not fall on
/// character boundaries of `text`, or that overlap an earlier one, are ignored.
pub fn highlighted(
    text: impl Into<SharedString>,
    ranges: &[Range<usize>],
    color: Hsla,
) -> StyledText {
    let text = text.into();
    let style = HighlightStyle {
        color: Some(color),
        font_weight: Some(FontWeight::BOLD),
        ..HighlightStyle::default()
    };
    let mut end = 0;
    let highlights = ranges
        .iter()
        .filter(|range| {
            let valid = range.start >= end
                && range.start < range.end
                && text.is_char_boundary(range.start)
                && text.is_char_boundary(range.end);
            if valid {
                end = range.end;
            }
            valid
        })
        .map(|range| (range.clone(), style))
        .collect::<Vec<_>>();
    StyledText::new(text).with_highlights(highlights)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(ranked: &[Ranked<&String>]) -> Vec<String> {
        ranked.iter().map(|entry| entry.item.clone()).collect()
    }

    #[test]
    fn basename_and_consecutive_matches_outrank_scattered_ones() {
        let files = [
            "src/main.rs",
            "src/map_index.rs",
            "main/resources/icon.svg",
            "src/domain.rs",
            "README.md",
        ]
        .map(str::to_owned);
        let ranked = rank(files.iter(), |path| path.as_str(), "mai", 100);
        assert_eq!(
            paths(&ranked),
            [
                "src/main.rs",
                "src/map_index.rs",
                "src/domain.rs",
                "main/resources/icon.svg"
            ]
        );
        assert_eq!(
            rank(files.iter(), |path| path.as_str(), "MAIN", 100)[0].item,
            "src/main.rs"
        );
        assert!(rank(files.iter(), |path| path.as_str(), "xyz", 100).is_empty());
    }

    #[test]
    fn multibyte_ranges_fall_on_character_boundaries() {
        let found = fuzzy("src/Éclair.rs", &normalize("ÉC"));
        assert_eq!(found.map(|found| found.ranges), Some(vec![4..6, 6..7]));
    }

    #[test]
    fn empty_query_keeps_shortest_keys_first() {
        let names = ["beta", "alpha", "be", "gamma-ray"];
        let ranked = rank(names, |name| *name, "", 3);
        assert_eq!(
            ranked.iter().map(|entry| entry.item).collect::<Vec<_>>(),
            ["be", "beta", "alpha"]
        );
        assert!(ranked.iter().all(|entry| entry.ranges.is_empty()));
    }

    #[test]
    fn limit_keeps_the_best_candidates_in_order() {
        let many: Vec<_> = (0..1000).map(|i| format!("src/file_{i:04}.rs")).collect();
        let limited = rank(many.iter(), |path| path.as_str(), "file", 100);
        assert_eq!(limited.len(), 100);
        assert_eq!(limited.first().map(|entry| entry.item), Some(&many[0]));
        assert_eq!(limited.last().map(|entry| entry.item), Some(&many[99]));
        assert!(rank(many.iter(), |path| path.as_str(), "file", 0).is_empty());
    }
}
