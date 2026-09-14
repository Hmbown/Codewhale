use super::*;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn missing_pdf_path_precedes_unavailable_helper() {
    let temporary = tempfile::tempdir().expect("tempdir");
    let input = temporary.path().join("missing.pdf");
    let missing = temporary.path().join("definitely-not-pdftotext");
    let error = read_pdf_if_detected(
        &input,
        None,
        super::super::pdf::PdfTextCommand::test(missing.as_os_str(), Duration::from_secs(1), None),
    )
    .await
    .expect_err("missing path must fail before the missing helper is launched");

    match error {
        ToolError::ExecutionFailed { message } => {
            assert!(message.contains("Failed to read"), "{message}");
            assert!(message.contains("missing.pdf"), "{message}");
        }
        other => panic!("expected ordinary read failure, got {other:?}"),
    }
}

#[tokio::test]
async fn read_file_missing_pdftotext_is_a_failed_typed_outcome() {
    let temporary = tempfile::tempdir().expect("tempdir");
    let missing = temporary.path().join("definitely-not-pdftotext");
    let input = temporary.path().join("input.pdf");
    std::fs::write(&input, b"%PDF-1.7\n%%EOF").expect("fixture");

    let error = read_pdf_with_command(
        &input,
        None,
        super::super::pdf::PdfTextCommand::test(missing.as_os_str(), Duration::from_secs(1), None),
    )
    .await
    .expect_err("missing helper must fail the tool call");
    let payload = match &error {
        ToolError::NotAvailable { message } => {
            serde_json::from_str::<Value>(message).expect("structured unavailable payload")
        }
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(payload["type"], "binary_unavailable");
    assert_eq!(
        crate::tools::spec::ToolExecutionOutcome::from_legacy(Err(error)).status,
        crate::tools::spec::ToolTerminalStatus::Failed
    );
}

/// C05 regression: the reader used to stop at a fixed 2 000 lines even when
/// the byte budget had barely been touched, fragmenting an ordinary file for
/// no reason. Bytes are now the only bound.
#[tokio::test]
async fn contract_read_returns_a_file_of_more_than_two_thousand_short_lines_whole() {
    let _workshop_guard = crate::tools::large_output_router::active_workshop_test_guard();
    let content = (0..5_000)
        .map(|index| format!("line-{index}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert!(
        content.len() < READ_DEFAULT_MAX_BYTES,
        "fixture fits the budget"
    );

    let window = contract_read_window(&content, READ_DEFAULT_MAX_BYTES);
    assert!(!window.truncated);
    assert_eq!(window.shown_lines, 5_000);
    assert_eq!(window.content, content);

    let temporary = tempfile::tempdir().expect("tempdir");
    std::fs::write(temporary.path().join("many.txt"), &content).expect("fixture");
    let context = ToolContext::new(temporary.path());
    let result = ReadFileTool::execute_contract_read(json!({"path": "many.txt"}), &context)
        .await
        .expect("read result");
    assert_eq!(result.content, content);
    assert!(
        !result.content.contains("[Showing lines"),
        "no truncation footer"
    );
}

#[tokio::test]
async fn contract_read_returns_an_ordinary_source_file_whole_without_a_footer() {
    let temporary = tempfile::tempdir().expect("tempdir");
    let content = "fn main() {\n    println!(\"hi\");\n}\n";
    std::fs::write(temporary.path().join("main.rs"), content).expect("fixture");
    let context = ToolContext::new(temporary.path());
    let result = ReadFileTool::execute_contract_read(json!({"path": "main.rs"}), &context)
        .await
        .expect("read result");
    assert_eq!(result.content, content);
}

#[test]
fn contract_read_byte_limit_keeps_only_complete_utf8_lines() {
    let first = "é".repeat(30_000);
    let second = "z".repeat(60_000);
    let window = contract_read_window(&format!("{first}\n{second}\n"), READ_DEFAULT_MAX_BYTES);
    assert!(window.truncated);
    assert_eq!(window.shown_lines, 1);
    assert_eq!(window.content, first);
    assert!(std::str::from_utf8(window.content.as_bytes()).is_ok());
}

#[test]
fn read_budget_precedence_is_request_then_workshop_then_default() {
    let _workshop_guard = crate::tools::large_output_router::active_workshop_test_guard();

    crate::tools::large_output_router::WorkshopConfig::install_active(None);
    assert_eq!(effective_read_max_bytes(None), READ_DEFAULT_MAX_BYTES);
    assert_eq!(effective_read_max_bytes(Some(250_000)), 250_000);
    // Above the model-requestable maximum clamps down instead of erroring.
    assert_eq!(
        effective_read_max_bytes(Some(READ_REQUEST_MAX_BYTES * 10)),
        READ_REQUEST_MAX_BYTES
    );
    // A request below the active baseline leaves the baseline in place.
    assert_eq!(effective_read_max_bytes(Some(10)), READ_DEFAULT_MAX_BYTES);

    crate::tools::large_output_router::WorkshopConfig::install_active(Some(
        &crate::tools::large_output_router::WorkshopConfig {
            read_result_max_bytes: Some(700_000),
            ..Default::default()
        },
    ));
    assert_eq!(effective_read_max_bytes(None), 700_000);
    assert_eq!(effective_read_max_bytes(Some(200_000)), 700_000);
    // The workshop override keeps the 2 MiB absolute ceiling.
    crate::tools::large_output_router::WorkshopConfig::install_active(Some(
        &crate::tools::large_output_router::WorkshopConfig {
            read_result_max_bytes: Some(READ_RESULT_ABSOLUTE_MAX_BYTES * 4),
            ..Default::default()
        },
    ));
    assert_eq!(
        effective_read_max_bytes(None),
        READ_RESULT_ABSOLUTE_MAX_BYTES
    );
    crate::tools::large_output_router::WorkshopConfig::install_active(None);
}

#[tokio::test]
async fn contract_read_max_bytes_raises_the_budget_for_one_call() {
    let _workshop_guard = crate::tools::large_output_router::active_workshop_test_guard();
    let temporary = tempfile::tempdir().expect("tempdir");
    let line = "y".repeat(199);
    let content = std::iter::repeat_n(line.as_str(), 1_500)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(content.len() > READ_DEFAULT_MAX_BYTES);
    assert!(content.len() < READ_REQUEST_MAX_BYTES);
    std::fs::write(temporary.path().join("wide.txt"), &content).expect("fixture");
    let context = ToolContext::new(temporary.path());

    let default_budget = ReadFileTool::execute_contract_read(json!({"path": "wide.txt"}), &context)
        .await
        .expect("default budget read");
    assert!(
        default_budget.content.contains("100000-byte output budget"),
        "{}",
        default_budget.content
    );

    let raised = ReadFileTool::execute_contract_read(
        json!({"path": "wide.txt", "max_bytes": 400_000}),
        &context,
    )
    .await
    .expect("raised budget read");
    assert_eq!(raised.content, content);

    // Above the hard maximum clamps down; the file still fits, so it is whole.
    let clamped = ReadFileTool::execute_contract_read(
        json!({"path": "wide.txt", "max_bytes": 9_000_000}),
        &context,
    )
    .await
    .expect("clamped budget read");
    assert_eq!(clamped.content, content);
}

#[tokio::test]
async fn contract_read_paginates_an_oversized_file_with_an_honest_budget_footer() {
    let _workshop_guard = crate::tools::large_output_router::active_workshop_test_guard();
    let temporary = tempfile::tempdir().expect("tempdir");
    // 999-byte lines: 100 of them plus their 99 separators are 99 999 bytes,
    // one under the 100 000-byte budget, so page one is exactly lines 1-100.
    let line = "z".repeat(999);
    let content = std::iter::repeat_n(line.as_str(), 2_000)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(temporary.path().join("big.txt"), &content).expect("fixture");
    let context = ToolContext::new(temporary.path());

    let first = ReadFileTool::execute_contract_read(json!({"path": "big.txt"}), &context)
        .await
        .expect("first page");
    let footer = first
        .content
        .rsplit_once("\n\n")
        .expect("footer present")
        .1
        .to_string();
    assert_eq!(
        footer,
        "[Showing lines 1-100 of 2000 (100000-byte output budget). Use offset=101 to continue, or max_bytes up to 500000 to read more per call.]"
    );
    let shown = first.content.rsplit_once("\n\n").expect("body").0;
    assert_eq!(shown.lines().count(), 100);

    // The named continuation offset is exact: page two starts on line 101.
    let second =
        ReadFileTool::execute_contract_read(json!({"path": "big.txt", "offset": 101}), &context)
            .await
            .expect("second page");
    assert!(
        second.content.starts_with(&line),
        "second page starts at the named offset"
    );
    assert!(
        second.content.contains("Use offset=201 to continue"),
        "{}",
        second.content
    );
}

#[tokio::test]
async fn contract_read_reports_huge_first_line_with_exact_bash_fallback() {
    let _workshop_guard = crate::tools::large_output_router::active_workshop_test_guard();
    let temporary = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temporary.path().join("huge.txt"),
        "x".repeat(READ_DEFAULT_MAX_BYTES + 1),
    )
    .expect("fixture");
    let context = ToolContext::new(temporary.path());
    let result = ReadFileTool::execute_contract_read(json!({"path": "huge.txt"}), &context)
        .await
        .expect("read result");
    assert_eq!(
        result.content,
        "[Line 1 is 97.7KB, exceeds the 100000-byte output budget for this call. Use bash: sed -n '1p' huge.txt | head -c 100000]"
    );
}

#[tokio::test]
async fn contract_read_offset_oob_and_limit_continuation_match_contract() {
    let temporary = tempfile::tempdir().expect("tempdir");
    std::fs::write(temporary.path().join("lines.txt"), "one\ntwo\nthree").expect("fixture");
    let context = ToolContext::new(temporary.path());

    let limited = ReadFileTool::execute_contract_read(
        json!({"path": "lines.txt", "offset": 2, "limit": 1}),
        &context,
    )
    .await
    .expect("limited read");
    assert_eq!(
        limited.content,
        "two\n\n[1 more lines in file. Use offset=3 to continue.]"
    );

    let error =
        ReadFileTool::execute_contract_read(json!({"path": "lines.txt", "offset": 4}), &context)
            .await
            .expect_err("offset beyond EOF");
    assert_eq!(
        error.to_string(),
        "Failed to execute tool: Offset 4 is beyond end of file (3 lines total)"
    );
}

#[tokio::test]
async fn contract_read_uses_magic_not_extension_for_images() {
    let temporary = tempfile::tempdir().expect("tempdir");
    std::fs::write(temporary.path().join("plain.png"), "ordinary text").expect("text fixture");
    std::fs::write(
        temporary.path().join("renamed.data"),
        crate::image_attach::tests::PNG_1X1,
    )
    .expect("image fixture");
    std::fs::write(
        temporary.path().join("truncated.png"),
        [b"\x89PNG\r\n\x1a\n".as_slice(), b"\0\0\0\rIHDR".as_slice()].concat(),
    )
    .expect("truncated image fixture");
    let context = ToolContext::new(temporary.path());

    let text = ReadFileTool::execute_contract_read(json!({"path": "plain.png"}), &context)
        .await
        .expect("fake extension remains text");
    assert_eq!(text.content, "ordinary text");
    let image = ReadFileTool::execute_contract_read(json!({"path": "renamed.data"}), &context)
        .await
        .expect("real image uses typed transport");
    assert_eq!(image.content_blocks.len(), 1);
    assert!(matches!(
        &image.content_blocks[0],
        codewhale_tools::ToolResultContentBlock::Image { mime_type, .. }
            if mime_type == "image/png"
    ));
    let truncated = ReadFileTool::execute_contract_read(json!({"path": "truncated.png"}), &context)
        .await
        .expect("invalid image retains an omission receipt");
    assert!(truncated.content_blocks.is_empty());
    assert!(truncated.content.contains("Image omitted"));
}

#[test]
fn contract_edit_preparation_accepts_string_and_legacy_recovery_forms() {
    let encoded = prepare_contract_edit_input(json!({
        "path": "doc.txt",
        "edits": "[{\"oldText\":\"a\",\"newText\":\"b\"}]"
    }))
    .expect("encoded edits");
    assert_eq!(encoded["edits"][0], json!({"oldText": "a", "newText": "b"}));

    let recovered = prepare_contract_edit_input(json!({
        "path": "doc.txt",
        "edits": {"malformed": true},
        "oldText": "a",
        "newText": "b"
    }))
    .expect("legacy recovery");
    assert_eq!(
        recovered["edits"],
        json!([{"oldText": "a", "newText": "b"}])
    );
    assert!(recovered.get("oldText").is_none());
    assert!(recovered.get("newText").is_none());
}

#[test]
fn contract_edit_fuzzy_normalization_preserves_untouched_lines() {
    let original = "untouched line  \nShe said “hello”—today.   \ntail  \n";
    let updated = apply_contract_edits(
        original,
        &[ContractEdit {
            index: 0,
            old_text: "She said \"hello\"-today.".to_string(),
            new_text: "She said hello.".to_string(),
        }],
        "doc.txt",
    )
    .expect("fuzzy edit");
    assert_eq!(updated, "untouched line  \nShe said hello.\ntail  \n");
}

#[tokio::test]
async fn contract_edit_preserves_bom_and_crlf_without_prior_read() {
    let temporary = tempfile::tempdir().expect("tempdir");
    let path = temporary.path().join("doc.txt");
    std::fs::write(&path, "\u{FEFF}alpha\r\nbeta\r\n").expect("fixture");
    let context = ToolContext::new(temporary.path());
    let result = EditFileTool::execute_contract_edits(
        json!({
            "path": "doc.txt",
            "edits": [{"oldText": "alpha\nbeta", "newText": "one\ntwo"}]
        }),
        &context,
    )
    .await
    .expect("edit");
    assert_eq!(
        result.content,
        "Successfully replaced 1 block(s) in doc.txt."
    );
    assert_eq!(
        std::fs::read(&path).expect("updated"),
        "\u{FEFF}one\r\ntwo\r\n".as_bytes()
    );
}

#[tokio::test]
async fn queued_parallel_contract_edits_preserve_both_changes() {
    let temporary = tempfile::tempdir().expect("tempdir");
    let path = temporary.path().join("doc.txt");
    std::fs::write(&path, "alpha\nbeta\ngamma\n").expect("fixture");
    let context = ToolContext::new(temporary.path());
    let first_context = context.clone();
    let second_context = context.clone();

    let first = tokio::spawn(async move {
        EditFileTool::execute_contract_edits(
            json!({"path": "doc.txt", "edits": [{"oldText": "alpha", "newText": "A"}]}),
            &first_context,
        )
        .await
    });
    let second = tokio::spawn(async move {
        EditFileTool::execute_contract_edits(
            json!({"path": "doc.txt", "edits": [{"oldText": "gamma", "newText": "G"}]}),
            &second_context,
        )
        .await
    });
    first.await.expect("first task").expect("first edit");
    second.await.expect("second task").expect("second edit");
    assert_eq!(
        std::fs::read_to_string(path).expect("updated"),
        "A\nbeta\nG\n"
    );
}

#[tokio::test]
async fn cancelled_queued_pi_write_never_starts() {
    let temporary = tempfile::tempdir().expect("tempdir");
    let context = ToolContext::new(temporary.path());
    let path = context.resolve_path("queued.txt").expect("resolved path");
    let held = file_mutation_lock(&path).expect("queue").lock_owned().await;
    let cancellation = CancellationToken::new();
    let queued_context = context.clone().with_cancel_token(cancellation.clone());
    let queued = tokio::spawn(async move {
        WriteFileTool::execute_contract_write(
            json!({"path": "queued.txt", "content": "must-not-land"}),
            &queued_context,
        )
        .await
    });
    tokio::task::yield_now().await;
    cancellation.cancel();
    let error = queued
        .await
        .expect("queued task")
        .expect_err("queued write must cancel");
    assert!(matches!(error, ToolError::Cancelled { .. }));
    assert!(!path.exists());
    drop(held);
}

#[cfg(unix)]
#[tokio::test]
async fn contract_edit_rejects_read_only_target_before_atomic_replace() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = tempfile::tempdir().expect("tempdir");
    let path = temporary.path().join("readonly.txt");
    std::fs::write(&path, "alpha\n").expect("fixture");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).expect("readonly");
    let context = ToolContext::new(temporary.path());
    let result = EditFileTool::execute_contract_edits(
        json!({"path": "readonly.txt", "edits": [{"oldText": "alpha", "newText": "beta"}]}),
        &context,
    )
    .await;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
        .expect("restore permissions");
    let error = result.expect_err("read-only target must fail");
    assert!(error.to_string().contains("readable and writable"));
    assert_eq!(std::fs::read_to_string(path).expect("unchanged"), "alpha\n");
}
