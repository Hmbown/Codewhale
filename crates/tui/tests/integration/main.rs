//! Consolidated harness for plain `#[test]`/`#[tokio::test]` acceptance suites.
//!
//! See `crates/tui/tests/README.md` for why this exists: 17 small integration
//! binaries each re-linked the full `codewhale-tui` graph — one harness keeps the
//! same test names (`integration::adaptive_evidence_acceptance::...`) so
//! `cargo test -p codewhale-tui adaptive_evidence_acceptance` still filters.

// Production modules that are `#[path]`-included by the test files below and
// that themselves use `crate::`. They must exist at the harness crate root so
// `crate::config`, `crate::shell_dispatcher`, etc. resolve when the same
// files are compiled as `crate::integration::<test>::<module>`.

#[path = "../../src/config/home.rs"]
#[allow(dead_code)]
mod config;
#[path = "../../src/eval.rs"]
mod eval;
#[path = "../../src/skills/install.rs"]
#[allow(dead_code)]
mod install;
#[path = "../support/llm_client.rs"]
mod llm_client;
#[path = "../../src/network_policy.rs"]
mod network_policy;
/// `network_policy.rs` resolves its audit file through `crate::audit`. The
/// harness has no audit module, so it gets a per-process scratch log: like the
/// production cfg(test) path (#6534), a test never appends to the real
/// `~/.codewhale/audit.log`.
mod audit {
    pub fn audit_log_path() -> Option<std::path::PathBuf> {
        Some(
            std::env::temp_dir()
                .join(format!("codewhale-it-audit-{}", std::process::id()))
                .join("audit.log"),
        )
    }
}
#[path = "../../src/skills/package_digest.rs"]
#[allow(dead_code)]
mod package_digest;
#[path = "../../src/shell_dispatcher.rs"]
mod shell_dispatcher;
// The legacy text tool-call parser lives in codewhale-core now; keep the
// `crate::tool_parser` path the suites use.
use codewhale_core::tool_parser;

mod adaptive_evidence_acceptance;
mod cache_guard;
mod coordination_acceptance;
mod diagnostic_read_only;
mod dotenv_authority;
mod eval_harness;
mod exec_persistent_service;
mod exec_stream_drop_acceptance;
mod exec_turn_usage;
mod integration_mock_llm;
mod issue_report_acceptance;
mod lifecycle_outbox_exec;
mod palette_audit;
mod protocol_recovery;
mod reasoning_content_replayed_after_tool_call;
mod shell_denial_acceptance;
mod skill_cli;
mod telemetry_contract;
mod verifiers_harness_contract;
mod workflow_tool_stream_acceptance;
