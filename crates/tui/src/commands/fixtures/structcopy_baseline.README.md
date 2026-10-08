# Structcopy preservation fixture

Captured through the real public command dispatcher at upstream commit
`6ce2d2e245fd89b93c232fbdf2ac3d6b6b8c12a5`, before FEAT-026 production edits,
on Linux with stable Rust 1.98.0 and nextest CI profile, hermetic HOME and
RUST_MIN_STACK=16777216. Source: session_structcopy_host_tests.rs fixture.
The temporary capture entry point was removed; ordinary assertions only read
this file. It must never be regenerated from the migrated implementation.

Only object.started_at_ms in the owned-workflow JSON string is normalized to
1700000000000, preserving its original 13-byte width and receipt byte counts.
Other values, full canonical JSON strings and localized receipts compare exactly.
The fixture also records clipboard contents and result error flags; test assertions
independently verify no actions, unchanged session/transcript/plan, duplicate-last
selection, busy-plan no-write and missing workflow journal non-creation.

Capture command and original source/log hashes are retained in the external
FEAT-026 MemoryBank evidence/P2 directory. Numeric test counts do not substitute
for these comparisons. A modified expected receipt is used as a negative control
and then restored byte-for-byte; ordinary tests never write their expectations.

## Current-main reconciliation (2026-09-30)

Upstream added explicit host execution identities and rejects duplicate or
inconsistent tool-call matches. The original fixture remains byte-identical.
The assertion replaces only the historical `tool_duplicate_last` expectation
with the existing tool-unavailable receipt. Tri-state fixtures now contain one
result each, rather than accumulating duplicates. The upstream tests
`tool_copy_selects_execution_when_provider_reuses_wire_id` and
`tool_copy_refuses_ambiguous_or_inconsistent_identity` move unchanged into
`session_structcopy_regression_tests.rs` and protect the new identity rules.

## Localization audit reconciliation (2026-10-01)

The integrated Chinese localization audit changed the turn label from `轮次`
to `回合` in three receipts and shortened the command description to
`将一个会话对象复制为已脱敏的 JSON；不是模型工具`. The assertion applies exactly
these four textual updates to the preserved capture. The fixture itself,
canonical JSON payloads, byte counts, English receipts, authority checks and
transport outcomes remain unchanged.
