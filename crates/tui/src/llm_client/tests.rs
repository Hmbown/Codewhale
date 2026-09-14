use super::*;

#[test]
fn missing_google_thought_signature_errors_explain_recovery() {
    for detail in [
        "Function call is missing a thought_signature in functionCall parts.",
        "Function call is missing thought_signature.",
        "The thought_signature is missing from the function call.",
    ] {
        for body in [
            detail.to_string(),
            serde_json::json!({"error": {"message": detail, "code": 400}}).to_string(),
            serde_json::json!({"error": "Bad Request", "message": detail}).to_string(),
        ] {
            let message = sanitize_http_error_body(Some("Custom"), 400, &body);
            assert!(message.contains("built-in `google` provider"), "{message}");
            assert!(message.contains("start a new session"), "{message}");
            assert!(message.contains(detail), "provider detail must survive");
            assert_eq!(sanitize_http_error_body(None, 400, &message), message);
            let error = LlmError::from_http_response(400, &message);
            assert!(matches!(
                error,
                LlmError::InvalidRequest { status: 400, .. }
            ));
            assert!(!error.is_retryable());
        }
    }
}

#[test]
fn google_thought_signature_hint_requires_a_missing_signature_400() {
    for (status, detail) in [
        (400, "Invalid model name"),
        (400, "Invalid thought_signature in functionCall parts"),
        (400, "Unsupported parameter: thought_signature"),
        (
            401,
            "Function call is missing a thought_signature in functionCall parts.",
        ),
        (
            429,
            "Function call is missing a thought_signature in functionCall parts.",
        ),
        (
            500,
            "Function call is missing a thought_signature in functionCall parts.",
        ),
    ] {
        let body = serde_json::json!({"error": {"message": detail}}).to_string();
        assert_eq!(
            sanitize_http_error_body(Some("Custom"), status, &body),
            detail
        );
    }
}

#[test]
fn google_thought_signature_hint_keeps_large_provider_errors_bounded() {
    let body = format!(
        "Function call is missing a thought_signature in functionCall parts. {}",
        "界".repeat(3_000)
    );
    let message = sanitize_http_error_body(None, 400, &body);
    assert!(message.contains("start a new session"));
    assert!(message.chars().count() < 2_000);
    assert_eq!(sanitize_http_error_body(None, 400, &message), message);
}

#[test]
fn google_thought_signature_hint_preserves_quota_and_html_handling() {
    let detail = "Function call is missing a thought_signature in functionCall parts.";
    let body = serde_json::json!({
        "error": {"message": detail, "code": "insufficient_quota"}
    })
    .to_string();
    let message = sanitize_http_error_body(None, 400, &body);
    assert!(matches!(
        LlmError::from_http_response(400, &message),
        LlmError::QuotaExhausted(_)
    ));
    let html = format!("<!doctype html><html><body>{detail}</body></html>");
    let message = sanitize_http_error_body(None, 400, &html);
    assert!(message.contains("HTML error page"));
    assert!(!message.contains("<html>"));
}

#[test]
fn retryability_distinguishes_transient_failures_from_durable_failures() {
    for error in [
        LlmError::RateLimited {
            message: "too many requests".into(),
            retry_after: None,
        },
        LlmError::ServerError {
            status: 500,
            message: "internal error".into(),
        },
        LlmError::NetworkError("connection refused".into()),
        LlmError::Timeout(Duration::from_secs(30)),
    ] {
        assert!(error.is_retryable(), "expected transient error: {error}");
    }
    for error in [
        LlmError::authentication_error("invalid key"),
        LlmError::AuthorizationError("blocked".into()),
        LlmError::InvalidRequest {
            status: 400,
            message: "bad json".into(),
        },
        LlmError::ContentPolicyError("unsafe content".into()),
        LlmError::ContextLengthError("too long".into()),
    ] {
        assert!(!error.is_retryable(), "expected durable error: {error}");
    }
}

#[test]
fn http_response_boundary_classifies_status_contract() {
    assert!(matches!(
        LlmError::from_http_response(429, "rate limit exceeded"),
        LlmError::RateLimited { .. }
    ));
    assert!(matches!(
        LlmError::from_http_response(401, "invalid api key"),
        LlmError::AuthenticationError(_)
    ));
    assert!(matches!(
        LlmError::from_http_response(403, "forbidden"),
        LlmError::AuthorizationError(_)
    ));
    assert!(matches!(
        LlmError::from_http_response(403, "invalid api key"),
        LlmError::AuthenticationError(_)
    ));
    let cancelled = LlmError::from_http_response(499, "upstream request cancelled");
    assert!(matches!(
        &cancelled,
        LlmError::ServerError { status: 499, .. }
    ));
    assert!(cancelled.is_retryable());
    assert!(matches!(
        LlmError::from_http_response(500, "internal server error"),
        LlmError::ServerError { status: 500, .. }
    ));
    assert!(matches!(
        LlmError::from_http_response(503, "service unavailable"),
        LlmError::ServerError { status: 503, .. }
    ));
    assert!(matches!(
        LlmError::from_http_response(400, "context_length_exceeded"),
        LlmError::ContextLengthError(_)
    ));
    assert!(matches!(
        LlmError::from_http_response(400, "content_policy_violation"),
        LlmError::ContentPolicyError(_)
    ));
    assert!(matches!(
        LlmError::from_http_response(400, "invalid json"),
        LlmError::InvalidRequest { status: 400, .. }
    ));
    // "Unsupported parameter: max_output_tokens" names a *token* field, which
    // the generic keyword rules misread as a context-window overflow. It is a
    // request-shape error, and retrying or compacting cannot fix it.
    assert!(matches!(
        LlmError::from_http_response(
            400,
            "{\"error\":{\"code\":\"unsupported_parameter\",\"message\":\"Unsupported parameter: max_output_tokens\"}}"
        ),
        LlmError::InvalidRequest { status: 400, .. }
    ));
    assert!(matches!(
        LlmError::from_http_response(
            400,
            "{\"error\":{\"type\":\"invalid_request_error\",\"message\":\"Unsupported parameter: temperature\"}}"
        ),
        LlmError::InvalidRequest { status: 400, .. }
    ));
}

#[test]
fn explicit_400_402_and_429_quota_responses_are_typed_and_non_retryable() {
    for (status, body) in [
        (
            400,
            r#"{"error":{"code":"insufficient_quota","message":"You exceeded your current quota"}}"#,
        ),
        (
            429,
            r#"{"error":{"type":"insufficient_quota","message":"Billing limit reached"}}"#,
        ),
        (
            402,
            r#"{"error":{"code":"billing_hard_limit_reached","message":"Payment required"}}"#,
        ),
        (
            429,
            "You exceeded your current quota. Please check your plan and billing details.",
        ),
        (429, "Account quota exhausted"),
    ] {
        let error = LlmError::from_http_response(status, body);
        assert!(matches!(error, LlmError::QuotaExhausted(_)));
        assert!(!error.is_retryable());
    }

    let raw = r#"{"error":{"code":"billing_hard_limit_reached","message":"Account unavailable"}}"#;
    let safe = sanitize_http_error_body(Some("fixture"), 429, raw);
    assert!(matches!(
        LlmError::from_http_response(429, &safe),
        LlmError::QuotaExhausted(_)
    ));
}

#[test]
fn generic_429_stays_rate_limited_and_retryable() {
    for body in [
        "Too Many Requests",
        "Rate limit on your API quota exceeded",
        "Requests per minute quota exceeded",
        "Quota rate limit exceeded; retry after 10 seconds",
    ] {
        let error = LlmError::from_http_response(429, body);
        assert!(
            matches!(error, LlmError::RateLimited { .. }),
            "expected transient rate limit for {body:?}, got {error:?}"
        );
        assert!(error.is_retryable());
    }

    let raw = r#"{"error":{"code":"RESOURCE_EXHAUSTED","message":"Rate limit on your API quota exceeded"}}"#;
    let safe = sanitize_http_error_body(Some("fixture"), 429, raw);
    let error = LlmError::from_http_response(429, &safe);
    assert!(matches!(error, LlmError::RateLimited { .. }));
    assert!(error.is_retryable());
}

#[test]
fn missing_google_signature_400_explains_recovery_without_widening_gateway_preflight() {
    let message = "Function call is missing a thought_signature in functionCall parts";
    for body in [
        message.to_string(),
        serde_json::json!({"error": {"message": message}}).to_string(),
        format!("{message} {}", "详情".repeat(2_000)),
    ] {
        let safe = sanitize_http_error_body(Some("OpenAI-compatible"), 400, &body);
        assert!(safe.contains(message));
        assert!(safe.contains("built-in `google` provider"));
        assert!(safe.contains("start a new session"));
        assert!(safe.contains("gateway"));
        assert!(
            safe.chars().count() <= 2_003,
            "error bound survives the hint"
        );
    }
    for (status, body) in [
        (429, message),
        (200, message),
        (400, "Invalid thought_signature"),
        (400, "Missing required parameter: model"),
    ] {
        assert_eq!(sanitize_http_error_body(None, status, body), body);
    }
}

#[tokio::test]
async fn retry_loop_stops_after_one_typed_quota_failure() {
    let mut calls = 0;
    let result: RetryResult<i32> = with_retry(
        &RetryConfig::default(),
        || {
            calls += 1;
            async {
                Err(LlmError::from_http_response(
                    429,
                    r#"{"error":{"code":"insufficient_quota"}}"#,
                ))
            }
        },
        None,
    )
    .await;
    assert_eq!(result.unwrap_err().attempts, 1);
    assert_eq!(calls, 1);
}

#[tokio::test]
async fn retry_loop_stops_after_one_authentication_failure() {
    let mut calls = 0;
    let result: RetryResult<i32> = with_retry(
        &RetryConfig::default(),
        || {
            calls += 1;
            async { Err(LlmError::authentication_error("bad key")) }
        },
        None,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(calls, 1);
}
