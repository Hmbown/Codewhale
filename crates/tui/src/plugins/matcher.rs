//! Pure keyword matcher over plugin metadata.
//!
//! Matches live as data: explicit `keywords`, `domains` (scheme / `www.` /
//! path stripped), and the plugin `name`. There is no `regex` dependency —
//! matching is substring search guarded by ASCII word boundaries.

use std::cmp::Reverse;

/// A plugin to match a draft against.
pub struct KeywordCandidate<'a> {
    pub name: &'a str,
    pub domains: &'a [String],
    pub keywords: &'a [String],
}

/// Return the candidate index and exact term that matched `draft`.
///
/// Returns `None` when `draft` has fewer than 3 characters or nothing matches.
/// Longer keywords take precedence; a keyword matches only when the occurrence
/// is flanked by ASCII word boundaries.
pub fn match_plugin_keyword(
    draft: &str,
    candidates: &[KeywordCandidate<'_>],
) -> Option<(usize, String)> {
    if draft.trim_start().starts_with('/') || draft.chars().count() < 3 {
        return None;
    }
    let draft_lc = draft.to_ascii_lowercase();
    let haystack = draft_lc.as_bytes();

    let mut pairs: Vec<(String, usize)> = Vec::new();
    for (idx, candidate) in candidates.iter().enumerate() {
        for keyword in effective_keywords(candidate) {
            pairs.push((keyword, idx));
        }
    }
    pairs.sort_by_key(|(keyword, _)| Reverse(keyword.len()));

    pairs
        .iter()
        .find(|(keyword, _)| keyword_matches(haystack, keyword.as_bytes()))
        .map(|(keyword, idx)| (*idx, keyword.clone()))
}

fn effective_keywords(candidate: &KeywordCandidate<'_>) -> Vec<String> {
    let mut keywords = Vec::new();
    for keyword in candidate.keywords {
        let normalized = keyword.trim().to_ascii_lowercase();
        if is_specific_term(&normalized) {
            keywords.push(normalized);
        }
    }
    for domain in candidate.domains {
        if let Some(normalized) = normalize_domain(domain)
            && is_specific_term(&normalized)
            && !matches!(
                normalized.as_str(),
                "github.com" | "gitlab.com" | "bitbucket.org"
            )
        {
            keywords.push(normalized);
        }
    }
    let name = candidate.name.trim().to_ascii_lowercase();
    if is_specific_term(&name) {
        keywords.push(name);
    }
    keywords
}

// Core vocabulary is not evidence that a user needs an integration. A
// specific product name, phrase or domain is still eligible.
fn is_specific_term(term: &str) -> bool {
    term.chars().count() >= 3
        && !term.chars().any(char::is_control)
        && !matches!(
            term,
            "mcp"
                | "plugin"
                | "plugins"
                | "skill"
                | "skills"
                | "agent"
                | "agents"
                | "tool"
                | "tools"
                | "data"
                | "code"
                | "model"
                | "models"
                | "session"
                | "sessions"
        )
}

pub(crate) fn normalize_domain(domain: &str) -> Option<String> {
    let trimmed = domain.trim();
    let after_scheme = match trimmed.find("://") {
        Some(i) => &trimmed[i + 3..],
        None => trimmed,
    };
    let host = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme)
        .to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn keyword_matches(haystack: &[u8], keyword: &[u8]) -> bool {
    if keyword.is_empty() {
        return false;
    }
    let len = haystack.len();
    haystack
        .windows(keyword.len())
        .enumerate()
        .any(|(start, window)| {
            if window != keyword {
                return false;
            }
            let end = start + keyword.len();
            let start_ok = start == 0 || is_word(haystack[start - 1]) != is_word(haystack[start]);
            let end_ok = end == len || is_word(haystack[end - 1]) != is_word(haystack[end]);
            start_ok && end_ok
        })
}

fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    // Existing selection tests assert the index; the receipt test below also
    // checks the matched term carried through to the UI.
    fn match_plugin_keyword(draft: &str, candidates: &[KeywordCandidate<'_>]) -> Option<usize> {
        super::match_plugin_keyword(draft, candidates).map(|(index, _)| index)
    }

    #[test]
    fn match_receipt_names_the_exact_trigger() {
        let keywords = vec!["finance".to_string(), "mcp".to_string()];
        let candidates = [candidate("kimi-datasource", &[], &keywords)];
        assert_eq!(
            super::match_plugin_keyword("help with finance", &candidates),
            Some((0, "finance".into()))
        );
        assert_eq!(
            super::match_plugin_keyword("help with mcp", &candidates),
            None
        );
    }

    fn candidate<'a>(
        name: &'a str,
        domains: &'a [String],
        keywords: &'a [String],
    ) -> KeywordCandidate<'a> {
        KeywordCandidate {
            name,
            domains,
            keywords,
        }
    }

    #[test]
    fn longest_keyword_takes_precedence() {
        let short = vec!["editor".to_string()];
        let long = vec!["code editor".to_string()];
        let candidates = [
            candidate("plugin-a", &[], &short),
            candidate("plugin-b", &[], &long),
        ];
        assert_eq!(
            match_plugin_keyword("my code editor rocks", &candidates),
            Some(1)
        );
    }

    #[test]
    fn word_boundary_required() {
        let keywords = vec!["box".to_string()];
        let candidates = [candidate("box", &[], &keywords)];
        assert_eq!(match_plugin_keyword("i love boxing", &candidates), None);
        assert_eq!(match_plugin_keyword("i love box", &candidates), Some(0));
    }

    #[test]
    fn boxing_does_not_match_box() {
        let keywords = vec!["box".to_string()];
        let candidates = [candidate("box", &[], &keywords)];
        assert_eq!(match_plugin_keyword("try boxing drills", &candidates), None);
    }

    #[test]
    fn domains_match_inside_pasted_urls() {
        let domains = vec!["figma.com".to_string()];
        let none: Vec<String> = Vec::new();
        let candidates = [candidate("design-app", &domains, &none)];
        assert_eq!(
            match_plugin_keyword("open https://www.figma.com/board/x please", &candidates),
            Some(0)
        );
        assert_eq!(
            match_plugin_keyword("open figma.com please", &candidates),
            Some(0)
        );
        assert_eq!(match_plugin_keyword("open figma please", &candidates), None);
    }

    #[test]
    fn name_is_used_as_fallback() {
        let none: Vec<String> = Vec::new();
        let candidates = [candidate("obsidian", &[], &none)];
        assert_eq!(
            match_plugin_keyword("open obsidian now", &candidates),
            Some(0)
        );
    }

    #[test]
    fn draft_below_min_length_never_matches() {
        let keywords = vec!["go".to_string()];
        let candidates = [candidate("go", &[], &keywords)];
        assert_eq!(match_plugin_keyword("go", &candidates), None);
        let git = vec!["git".to_string()];
        let candidates = [candidate("git", &[], &git)];
        assert_eq!(match_plugin_keyword("git", &candidates), Some(0));
    }

    #[test]
    fn core_vocabulary_short_claims_and_commands_do_not_trigger_suggestions() {
        let words = [
            "mcp", "plugin", "plugins", "skill", "skills", "agent", "agents", "tool", "tools",
            "code", "data", "model", "models", "session", "sessions", "go", "ai",
        ];
        let keywords = words
            .iter()
            .map(|word| word.to_string())
            .collect::<Vec<_>>();
        let candidates = [candidate("mcp", &[], &keywords)];
        for word in words {
            assert_eq!(
                match_plugin_keyword(&format!("please help with {word}"), &candidates),
                None,
                "{word}"
            );
        }
        let shared_host = vec!["https://github.com/example/plugin".to_string()];
        let candidates = [candidate("supabase", &shared_host, &[])];
        assert_eq!(
            match_plugin_keyword("open github.com/example/repo", &candidates),
            None
        );
        for command in ["/mcp", "  /plugin show supabase", "/skills supabase"] {
            assert_eq!(match_plugin_keyword(command, &candidates), None);
        }
        assert_eq!(
            match_plugin_keyword("add supabase auth", &candidates),
            Some(0)
        );
    }
}
