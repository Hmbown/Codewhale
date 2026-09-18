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
//! Interim exception, recorded not hidden (#6088): `acp_server.rs` runs its
//! own agentic tool loop (`run_agentic_prompt_turn`) for ACP IDE sessions,
//! which do not run on the full thread/turn runtime yet. #5835 (IDE stage 2)
//! converges them onto `Engine::run_turn` and deletes this exception along
//! with the loop. Until then the scan below asserts the exception set is
//! exactly these two owners — a third loop fails the same way a second
//! used to.

use std::path::{Path, PathBuf};

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

    let mut found = Vec::new();
    let mut excepted = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("async fn run_turn")
                || trimmed.starts_with("pub async fn run_turn")
                || trimmed.starts_with("pub(crate) async fn run_turn")
                || trimmed.starts_with("pub(super) async fn run_turn")
            {
                found.push((
                    file.strip_prefix(&root).unwrap_or(file).to_path_buf(),
                    idx + 1,
                ));
            }
            // Interim #6088 exception: the ACP IDE loop, matched by its own
            // name so it cannot hide behind the `run_turn` spelling.
            if trimmed.starts_with("async fn run_agentic_prompt_turn")
                || trimmed.starts_with("pub(crate) async fn run_agentic_prompt_turn")
            {
                excepted.push((
                    file.strip_prefix(&root).unwrap_or(file).to_path_buf(),
                    idx + 1,
                ));
            }
        }
    }

    assert_eq!(
        found.len(),
        1,
        "expected exactly one turn loop in the workspace, found {}: {found:#?}\n\
         A second `run_turn` means two implementations of the agent loop. If the \
         runtime is being migrated, move the one that exists rather than adding \
         another beside it.",
        found.len()
    );
    let expected_owner = Path::new("crates")
        .join("tui")
        .join("src")
        .join("core")
        .join("engine")
        .join("turn_loop.rs");
    let (owner_path, owner_line) = &found[0];
    assert_eq!(
        owner_path,
        &expected_owner,
        "the turn loop moved to {}:{} — update this guard and docs/ARCHITECTURE.md \
         together so the documented owner stays true",
        owner_path.display(),
        owner_line
    );
    let expected_exception = Path::new("crates")
        .join("tui")
        .join("src")
        .join("acp_server.rs");
    assert_eq!(
        excepted.len(),
        1,
        "expected exactly one recorded #6088 exception (acp_server's agentic \
         loop), found {}: {excepted:#?}\n\
         A second `run_agentic_prompt_turn` is a third turn loop — converge it \
         onto Engine::run_turn instead. If #5835 deleted the ACP loop, delete \
         this exception with it.",
        excepted.len()
    );
    assert_eq!(
        &excepted[0].0,
        &expected_exception,
        "the #6088 exception moved to {} — update this guard, #6088, and #5835 \
         together so the recorded owner stays true",
        excepted[0].0.display(),
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
