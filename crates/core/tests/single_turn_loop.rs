//! The workspace must contain exactly one turn loop.
//!
//! `crates/core` carried a placeholder `engine/` tree whose `Engine::run`
//! accepted `Op::SendMessage`, appended to a journal, and emitted
//! `TurnComplete { status: "completed" }` without ever contacting a model. It
//! had no callers, but its doc comments ("the real turn loop is wired here in
//! the next slice") were load-bearing for `docs/ARCHITECTURE.md`'s claim that
//! core owns the agent loop, and a reader could reasonably have built on it.
//!
//! This guard is deliberately a source scan rather than a type check: the thing
//! being prevented is a *second implementation*, which by definition would not
//! be reachable from the first.
//!
//! # What changed, and why (#6242)
//!
//! Until #6242 this scan matched a function *name* — `async fn run_turn`. A
//! name is not the rule. `acp_server.rs` grew a full second turn loop called
//! `run_agentic_prompt_turn`, and the guard stayed green for as long as it
//! existed; nobody picked that name to evade anything, which is exactly the
//! problem. A guard satisfied by spelling is satisfied by accident.
//!
//! So the scan now matches the *shape*. A turn loop is a loop whose body does
//! all three of these in one iteration:
//!
//! 1. **drives a model** — calls something that opens or consumes a provider
//!    message stream / completion (an identifier ending in `stream`,
//!    `create_message`, `completion`, or `complete`, called as a function);
//! 2. **dispatches tool calls** — calls something that executes tools
//!    (`execute_*tool*`, `dispatch_*tool*`, `run_*tool*`, …);
//! 3. **assembles its own prompt** — pushes onto a message history
//!    (`…messages…`/`…history…`/`…conversation…`/`…prompt…`.`push`/`extend`).
//!
//! Anything doing all three per iteration *is* a turn loop, whatever it is
//! called. Renaming it does not hide it; only [`ALLOWED_TURN_LOOPS`] does, and
//! that list is read back at the end of the test so a stale entry fails too.
//!
//! # Known limitations
//!
//! - It is a lexical scan, not a type check. A turn loop that reaches the
//!   provider through an indirection matching none of the markers above, or
//!   that never mutates a message history, would not be seen. The markers are
//!   deliberately broad rather than exact for that reason, and
//!   [`detector_sees_a_renamed_turn_loop`] pins the detector against going
//!   vacuously blind.
//! - `#[cfg(test)]` modules and `tests/`, `benches/`, `examples/` sources are
//!   skipped: a test harness that drives rounds is not a shipped turn loop.
//! - It reports one loop per (file, enclosing function). A function with two
//!   turn loops in it is one finding, not two.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A loop that is permitted to drive model/tool rounds, and the reason.
///
/// Every entry must be *named*, *documented*, and *reachable* — an entry whose
/// loop no longer exists fails the test, so an exception has to be consciously
/// renewed instead of quietly outliving its reason.
struct AllowedTurnLoop {
    /// Workspace-relative path, `/`-separated.
    path: &'static str,
    /// Name of the function that lexically encloses the loop.
    owner: &'static str,
    /// Why this loop is allowed to exist. Exceptions cite their issue.
    why: &'static str,
}

const ALLOWED_TURN_LOOPS: &[AllowedTurnLoop] = &[
    AllowedTurnLoop {
        path: "crates/tui/src/core/engine/turn_loop.rs",
        owner: "run_turn",
        why: "THE turn loop. `Engine::run_turn` is the single agent loop; \
              docs/ARCHITECTURE.md and AGENTS.md both name it as the owner.",
    },
    AllowedTurnLoop {
        path: "crates/tui/src/acp_server.rs",
        owner: "run_agentic_prompt_turn",
        why: "INTERIM EXCEPTION (#6088). ACP IDE sessions do not run on the \
              full thread/turn runtime yet, so `run_agentic_prompt_turn` \
              drives its own bounded tool-round loop. #5835 (IDE stage 2) \
              converges them onto `Engine::run_turn`; when it lands, delete \
              the loop and this entry together.",
    },
];

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// A call to an identifier ending in one of these, i.e. "this iteration talks
/// to a model".
const MODEL_CALL_SUFFIXES: &[&str] = &["stream", "create_message", "completion", "complete"];

/// Prefix/infix pairs for "this iteration dispatches tool calls".
const TOOL_DISPATCH_VERBS: &[&str] = &[
    "execute", "dispatch", "run", "invoke", "perform", "handle", "call",
];

/// Receiver-name fragments for "this iteration assembles its own prompt".
const HISTORY_RECEIVER_FRAGMENTS: &[&str] = &["messages", "history", "conversation", "prompt"];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TurnLoopSite {
    path: String,
    owner: String,
    line: usize,
    keyword: String,
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Blank out comments, string literals, and char literals, preserving byte
/// offsets and newlines. Brace matching is only trustworthy over source that
/// cannot contain a `{` inside a comment or a `"{"` literal.
fn sanitize(src: &str) -> String {
    let bytes: Vec<char> = src.chars().collect();
    let mut out: Vec<char> = bytes.clone();
    let blank = |out: &mut Vec<char>, from: usize, to: usize| {
        for slot in out.iter_mut().take(to.min(bytes.len())).skip(from) {
            if *slot != '\n' {
                *slot = ' ';
            }
        }
    };
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        let next = bytes.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            let start = i;
            while i < bytes.len() && bytes[i] != '\n' {
                i += 1;
            }
            blank(&mut out, start, i);
        } else if c == '/' && next == Some('*') {
            let start = i;
            let mut depth = 0usize;
            while i < bytes.len() {
                if bytes[i] == '/' && bytes.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == '*' && bytes.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            blank(&mut out, start, i);
        } else if c == 'r' && matches!(next, Some('"') | Some('#')) {
            // Raw string `r"..."` / `r#"..."#`, but not the identifier `red`.
            let mut hashes = 0usize;
            let mut j = i + 1;
            while bytes.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if bytes.get(j) != Some(&'"') || (i > 0 && is_ident_char(bytes[i - 1])) {
                i += 1;
                continue;
            }
            let start = i;
            j += 1;
            loop {
                if j >= bytes.len() {
                    break;
                }
                if bytes[j] == '"' {
                    let closing = (1..=hashes).all(|k| bytes.get(j + k) == Some(&'#'));
                    if closing {
                        j += hashes + 1;
                        break;
                    }
                }
                j += 1;
            }
            i = j;
            blank(&mut out, start, i);
        } else if c == '"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == '\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            blank(&mut out, start, i);
        } else if c == '\'' {
            // `'a'` / `'\n'` are char literals; `'a` alone is a lifetime.
            let is_char_lit = if bytes.get(i + 1) == Some(&'\\') {
                true
            } else {
                bytes.get(i + 2) == Some(&'\'')
            };
            if is_char_lit {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == '\\' {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == '\'' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                blank(&mut out, start, i);
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out.into_iter().collect()
}

/// Strip `#[cfg(test)]`-gated items. A test harness that drives rounds is not
/// a shipped turn loop.
fn strip_cfg_test(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = chars.clone();
    let mut search_from = 0usize;
    let needle = "cfg(test)";
    while let Some(rel) = src[search_from..].find(needle) {
        let at = search_from + rel;
        search_from = at + needle.len();
        // Require a `#[` immediately before (allowing whitespace).
        let prefix = &src[..at];
        let Some(hash) = prefix.rfind("#[") else {
            continue;
        };
        if !prefix[hash + 2..].trim().is_empty() {
            continue;
        }
        let hash_ci = src[..hash].chars().count();
        let mut i = src[..search_from].chars().count();
        while i < chars.len() && chars[i] != '{' && chars[i] != ';' {
            i += 1;
        }
        if i >= chars.len() || chars[i] == ';' {
            continue;
        }
        let mut depth = 0usize;
        let mut j = i;
        while j < chars.len() {
            if chars[j] == '{' {
                depth += 1;
            } else if chars[j] == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            j += 1;
        }
        for slot in out.iter_mut().take((j + 1).min(chars.len())).skip(hash_ci) {
            if *slot != '\n' {
                *slot = ' ';
            }
        }
    }
    out.into_iter().collect()
}

/// Every `loop`/`for … in …`/`while …` block, as (keyword, body char range).
fn loop_bodies(chars: &[char]) -> Vec<(String, usize, usize)> {
    let mut found = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if !is_ident_char(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_ident_char(chars[i]) {
            i += 1;
        }
        if start > 0 && is_ident_char(chars[start - 1]) {
            continue;
        }
        let word: String = chars[start..i].iter().collect();
        if word != "loop" && word != "for" && word != "while" {
            continue;
        }
        // Walk to the body `{`, tracking paren/bracket depth so `for x in v[0] {`
        // and `while let Some(x) = it.next() {` resolve correctly.
        let mut j = i;
        let mut nesting = 0i32;
        let mut body_open = None;
        let mut saw_in = false;
        while j < chars.len() {
            match chars[j] {
                '(' | '[' => nesting += 1,
                ')' | ']' => nesting -= 1,
                ';' if nesting == 0 => break,
                '{' if nesting == 0 => {
                    body_open = Some(j);
                    break;
                }
                _ => {}
            }
            if word == "for"
                && nesting == 0
                && chars[j] == 'i'
                && chars.get(j + 1) == Some(&'n')
                && !is_ident_char(chars[j - 1])
                && chars.get(j + 2).is_some_and(|c| !is_ident_char(*c))
            {
                saw_in = true;
            }
            j += 1;
        }
        let Some(open) = body_open else { continue };
        // `impl Trait for Type {` and `for<'a> Fn(..)` are the `for` keyword
        // without a loop. A `for` loop always has an `in` before its body.
        if word == "for" && !saw_in {
            continue;
        }
        let mut depth = 0usize;
        let mut k = open;
        while k < chars.len() {
            if chars[k] == '{' {
                depth += 1;
            } else if chars[k] == '}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            k += 1;
        }
        found.push((word, open, k.min(chars.len().saturating_sub(1))));
    }
    found
}

/// True when `body` calls an identifier ending in any of `suffixes`.
fn calls_identifier_ending_in(body: &str, suffixes: &[&str]) -> bool {
    suffixes.iter().any(|suffix| {
        body.match_indices(suffix).any(|(at, _)| {
            let after = &body[at + suffix.len()..];
            after.trim_start().starts_with('(') && !after.starts_with(|c: char| is_ident_char(c))
        })
    })
}

/// True when `body` calls something like `execute_tool_calls(…)`.
fn dispatches_tool_calls(body: &str) -> bool {
    TOOL_DISPATCH_VERBS.iter().any(|verb| {
        body.match_indices(verb).any(|(at, _)| {
            if at > 0 && is_ident_char(body[..at].chars().next_back().unwrap_or(' ')) {
                return false;
            }
            let rest = &body[at + verb.len()..];
            if !rest.starts_with('_') {
                return false;
            }
            let ident_end = rest.find(|c: char| !is_ident_char(c)).unwrap_or(rest.len());
            rest[..ident_end].contains("tool")
        })
    })
}

/// True when `body` pushes onto something whose name reads like a prompt or
/// message history — the loop assembling its own conversation.
fn assembles_prompt_history(body: &str) -> bool {
    ["push", "extend"].iter().any(|method| {
        body.match_indices(method).any(|(at, _)| {
            let after = &body[at + method.len()..];
            if !after.trim_start().starts_with('(') {
                return false;
            }
            let before = body[..at].trim_end();
            let Some(before) = before.strip_suffix('.') else {
                return false;
            };
            let receiver_end = before.trim_end();
            let ident_start = receiver_end
                .rfind(|c: char| !is_ident_char(c))
                .map_or(0, |i| i + 1);
            let receiver = &receiver_end[ident_start..];
            HISTORY_RECEIVER_FRAGMENTS
                .iter()
                .any(|fragment| receiver.contains(fragment))
        })
    })
}

/// Name of the function that lexically encloses char offset `at`.
fn enclosing_fn(chars: &[char], at: usize) -> String {
    let head: String = chars[..at].iter().collect();
    let mut best = None;
    let mut search = 0usize;
    while let Some(rel) = head[search..].find("fn ") {
        let idx = search + rel;
        search = idx + 3;
        let before_ok = idx == 0 || !is_ident_char(head[..idx].chars().next_back().unwrap_or(' '));
        if !before_ok {
            continue;
        }
        let rest = head[idx + 3..].trim_start();
        let name_end = rest.find(|c: char| !is_ident_char(c)).unwrap_or(rest.len());
        if name_end > 0 {
            best = Some(rest[..name_end].to_string());
        }
    }
    best.unwrap_or_else(|| "<unknown>".to_string())
}

/// Find every turn loop in one Rust source, by shape.
fn detect_turn_loops(src: &str, rel_path: &str) -> Vec<TurnLoopSite> {
    let prepared = sanitize(&strip_cfg_test(src));
    let chars: Vec<char> = prepared.chars().collect();
    let mut sites: Vec<TurnLoopSite> = Vec::new();
    for (keyword, open, close) in loop_bodies(&chars) {
        let body: String = chars[open..=close.max(open)].iter().collect();
        if !calls_identifier_ending_in(&body, MODEL_CALL_SUFFIXES) {
            continue;
        }
        if !dispatches_tool_calls(&body) {
            continue;
        }
        if !assembles_prompt_history(&body) {
            continue;
        }
        let owner = enclosing_fn(&chars, open);
        let line = chars[..open].iter().filter(|c| **c == '\n').count() + 1;
        // One finding per (file, enclosing function): nested loops inside one
        // turn loop are the same turn loop.
        if sites.iter().any(|s| s.owner == owner) {
            continue;
        }
        sites.push(TurnLoopSite {
            path: rel_path.to_string(),
            owner,
            line,
            keyword,
        });
    }
    sites
}

// ---------------------------------------------------------------------------
// Source discovery
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    // crates/core/tests -> crates/core -> crates -> <root>
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root above crates/core")
        .to_path_buf()
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name == "target" || name == ".git" || name == "node_modules" {
                continue;
            }
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Shipped source only: test, bench, and example trees drive rounds on
/// purpose.
fn is_shipped_source(rel: &Path) -> bool {
    let components: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let Some((file, dirs)) = components.split_last() else {
        return false;
    };
    if dirs
        .iter()
        .any(|d| d == "tests" || d == "benches" || d == "examples")
    {
        return false;
    }
    !(file == "tests.rs" || file.ends_with("_tests.rs"))
}

fn rel_slash(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn workspace_declares_exactly_one_turn_loop() {
    let root = workspace_root();
    let crates = root.join("crates");
    assert!(crates.is_dir(), "expected {} to exist", crates.display());

    let mut files = Vec::new();
    rust_sources(&crates, &mut files);
    assert!(
        files.len() > 100,
        "source scan found too few files to trust"
    );

    let mut scanned = 0usize;
    let mut sites: Vec<TurnLoopSite> = Vec::new();
    for file in &files {
        let rel = file.strip_prefix(&root).unwrap_or(file).to_path_buf();
        if !is_shipped_source(&rel) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        scanned += 1;
        sites.extend(detect_turn_loops(&text, &rel_slash(&root, file)));
    }
    assert!(
        scanned > 100,
        "only {scanned} shipped sources scanned — the exclusion rules are \
         swallowing the workspace, so a pass here proves nothing"
    );
    sites.sort();

    let allowed: BTreeSet<(&str, &str)> = ALLOWED_TURN_LOOPS
        .iter()
        .map(|entry| (entry.path, entry.owner))
        .collect();

    let unlisted: Vec<&TurnLoopSite> = sites
        .iter()
        .filter(|site| !allowed.contains(&(site.path.as_str(), site.owner.as_str())))
        .collect();
    assert!(
        unlisted.is_empty(),
        "found {} turn loop(s) that are not on ALLOWED_TURN_LOOPS:\n{}\n\n\
         A turn loop is a loop that, in one iteration, drives a model stream, \
         dispatches tool calls, and appends to its own prompt history. There \
         is supposed to be exactly one of those — `Engine::run_turn`. If you \
         are migrating the runtime, move the loop that exists instead of \
         adding another beside it. If this genuinely must exist for now, add \
         a named, documented ALLOWED_TURN_LOOPS entry citing the issue that \
         deletes it, so the exception is visible and has to be renewed.",
        unlisted.len(),
        unlisted
            .iter()
            .map(|s| format!(
                "  - {}:{} (`{}`, `{}` loop)",
                s.path, s.line, s.owner, s.keyword
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    // The allowlist is read back: an entry whose loop is gone (or renamed, or
    // moved) fails, so no exception outlives its reason unnoticed.
    let detected: BTreeSet<(&str, &str)> = sites
        .iter()
        .map(|site| (site.path.as_str(), site.owner.as_str()))
        .collect();
    let stale: Vec<&AllowedTurnLoop> = ALLOWED_TURN_LOOPS
        .iter()
        .filter(|entry| !detected.contains(&(entry.path, entry.owner)))
        .collect();
    assert!(
        stale.is_empty(),
        "ALLOWED_TURN_LOOPS has {} entr(y/ies) with no matching turn loop in \
         the tree:\n{}\n\n\
         Either the loop was deleted (good — delete the entry too), or it \
         moved/was renamed (update the entry), or the detector stopped seeing \
         it, which is the #6242 failure all over again and must be fixed \
         rather than papered over.",
        stale.len(),
        stale
            .iter()
            .map(|e| format!("  - {} :: {} — {}", e.path, e.owner, e.why))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// The detector must find a turn loop it has never been told the name of.
///
/// This is the regression for #6242: the old guard matched `run_turn` and was
/// green for as long as `run_agentic_prompt_turn` existed. If the shape match
/// ever degrades back into a name match, this fails.
#[test]
fn detector_sees_a_renamed_turn_loop() {
    let src = r#"
        async fn absolutely_not_called_run_turn(&mut self) -> Result<()> {
            let mut messages = self.history.clone();
            loop {
                let stream = self.client.create_message_stream(request).await?;
                let (text, calls) = self.consume(stream).await?;
                messages.push(Message::assistant(text));
                if calls.is_empty() {
                    return Ok(());
                }
                let results = self.execute_tool_calls(calls).await?;
                messages.extend(results);
            }
        }
    "#;
    let sites = detect_turn_loops(src, "crates/whatever/src/sneaky.rs");
    assert_eq!(
        sites.len(),
        1,
        "shape detector missed a turn loop that avoids the name `run_turn`: {sites:#?}"
    );
    assert_eq!(sites[0].owner, "absolutely_not_called_run_turn");

    // …and must not fire on a loop that only does part of the job.
    let not_a_turn_loop = r#"
        async fn render(&mut self) -> Result<()> {
            for event in events {
                messages.push(event.text);
                self.paint(event);
            }
            Ok(())
        }
    "#;
    assert!(
        detect_turn_loops(not_a_turn_loop, "crates/whatever/src/ui.rs").is_empty(),
        "shape detector fired on a loop that never drives a model or tools"
    );
}

#[test]
fn core_does_not_reintroduce_a_placeholder_engine_module() {
    let core_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(
        !core_src.join("engine").exists(),
        "crates/core/src/engine/ is back. It was removed in v0.9.11 because it \
         emitted TurnComplete without calling a model and had no consumers; a \
         boundary type that does real work belongs in a named module, not a \
         second `engine`."
    );
}
