//! The account Work journey: assign Work, cancel it, read its outcome, quote
//! and launch bounded Boat trial compute, and connect a GitHub repository.
//!
//! Everything here is a thin, honest client of the account control plane.
//! Remote text is untrusted: it only reaches the terminal through `printable`,
//! ids are re-validated before they enter a URL path, and a mutating request
//! whose reply is lost is reported as an *unknown* outcome with the exact
//! command that is safe to run next, never as a failure or a success.

use serde_json::{Value, json};

use super::*;

/// The bounded Boat trial this CLI can start. These are the contract values
/// the control plane admits; anything else is refused by the server, so the
/// CLI states them instead of exposing knobs that could only fail.
const BOAT_TRIAL_SKU: &str = "boat-small";
const BOAT_TRIAL_SECONDS: u64 = 300;
const BOAT_TRIAL_MODEL_PROVIDER: &str = "deepseek";
const BOAT_TRIAL_MODEL: &str = "deepseek-flash";
const MAX_CONFIRMATION_BYTES: usize = 4096;
const MAX_CANCEL_REASON_CHARS: usize = 500;

fn valid_confirmation(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= MAX_CONFIRMATION_BYTES
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// Consume a pipe rather than placing signed consent proof in process argv.
/// Bound the read before decoding and allow only one terminal line ending.
pub(super) fn read_confirmation(reader: impl Read) -> Result<String> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_CONFIRMATION_BYTES as u64 + 3)
        .read_to_end(&mut bytes)
        .context("Could not read the launch confirmation from stdin")?;
    if bytes.len() > MAX_CONFIRMATION_BYTES + 2 {
        bail!("Launch confirmation from stdin is too long");
    }
    let value = String::from_utf8(bytes).context("Launch confirmation from stdin must be UTF-8")?;
    let token = value
        .strip_suffix("\r\n")
        .or_else(|| value.strip_suffix('\n'))
        .unwrap_or(&value);
    if !valid_confirmation(token) {
        bail!("Launch confirmation from stdin must be one non-empty confirmation token");
    }
    Ok(token.to_string())
}

fn at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |cursor, key| cursor.get(*key))
}

fn str_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    at(value, path).and_then(Value::as_str)
}

/// A sanitized, non-empty string from a remote document.
fn text_at(value: &Value, path: &[&str]) -> Option<String> {
    str_at(value, path)
        .map(printable)
        .filter(|text| !text.is_empty())
}

fn count_at(value: &Value, path: &[&str]) -> Option<u64> {
    at(value, path).and_then(Value::as_u64)
}

fn http_code(err: &anyhow::Error) -> Option<&str> {
    err.downcast_ref::<CloudHttpError>()
        .and_then(CloudHttpError::code)
}

/// A 2xx reply whose body cannot be read: the service acted, but the client
/// cannot say how, so it is an unknown outcome rather than a failure.
fn parse_reply(body: &[u8], what: &'static str) -> Result<Value> {
    serde_json::from_slice(body).map_err(|source| CloudTransportError::new(what, source).into())
}

fn validate_work_uuid(value: &str) -> Result<&str> {
    let id = value.trim();
    let groups = id.split('-').map(str::len).collect::<Vec<_>>();
    if groups != [8, 4, 4, 4, 12]
        || !id
            .bytes()
            .all(|byte| byte == b'-' || byte.is_ascii_hexdigit())
    {
        bail!(
            "Work ID must be the UUID printed when the Work was created (see `codewhale account agents work-status`)"
        );
    }
    Ok(id)
}

fn github_owner_ok(owner: &str) -> bool {
    (1..=39).contains(&owner.len())
        && owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !owner.starts_with('-')
        && !owner.ends_with('-')
}

fn github_name_ok(name: &str) -> bool {
    (1..=100).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !matches!(name, "." | "..")
}

fn validate_github_repo(value: &str) -> Result<String> {
    let value = value.trim();
    match value.split_once('/') {
        Some((owner, name)) if github_owner_ok(owner) && github_name_ok(name) => {
            Ok(value.to_string())
        }
        _ => bail!("Repository must be written OWNER/REPO, for example `octo-org/app`"),
    }
}

/// A draft-PR link is shown only when it is exactly a GitHub pull-request URL
/// (and, when the result names its repository, points into that repository).
/// A remote string that merely looks like a link is never echoed.
fn github_pull_request_url(url: &str, repository: &str) -> Option<String> {
    let rest = url.strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let (owner, name, kind, number) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some()
        || kind != "pull"
        || !github_owner_ok(owner)
        || !github_name_ok(name)
        || number.is_empty()
        || number.len() > 10
        || number.starts_with('0')
        || !number.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    if !repository.is_empty() && !repository.eq_ignore_ascii_case(&format!("{owner}/{name}")) {
        return None;
    }
    Some(url.to_string())
}

fn write_work_item<W: Write>(out: &mut W, item: &AccountAgentWork) -> Result<()> {
    writeln!(out, "Work ID: {}", item.id)?;
    writeln!(out, "Status: {}", printable(&item.status))?;
    if !item.objective.is_empty() {
        writeln!(out, "Objective: {}", printable(&item.objective))?;
    }
    Ok(())
}

/// `codewhale account agents work`: say what the Agent actually did with the
/// message. Only an actionable or queued reading creates Work; a control verb
/// or correction acts on Work that already exists, and a question changes
/// nothing, so those must never read as "Work is recorded".
pub(super) fn assign<T: CloudTransport, W: Write>(
    client: &CloudClient<'_, T>,
    out: &mut W,
    agent: &AccountAgent,
    objective: &str,
    message_id: &str,
) -> Result<()> {
    let message_id = validate_operation_key(message_id)?;
    let receipt = client
        .assign_agent_work(&agent.id, objective, message_id)
        .map_err(|err| {
            if outcome_unknown(&err) {
                err.context(format!(
                    "The request may or may not have reached Codewhale. Re-run this exact command with --message-id {message_id}; replaying the same --message-id never creates a second Work for the same instruction. If the message was a stop or a correction, check `work-status` first"
                ))
            } else {
                err
            }
        })?;
    let mut items = Vec::new();
    for item in receipt.work.iter().chain(receipt.queued_work.iter()) {
        validate_resource_id(&item.id, "Work")?;
        if item.agent_id != agent.id {
            bail!("The Codewhale service returned Work for a different Agent");
        }
        items.push(item);
    }
    let intent = receipt.intent.as_str();
    if items.is_empty() {
        match intent {
            "actionable" | "queued" => bail!(
                "The Codewhale service read this message as {} but returned no Work record, so nothing is confirmed. Re-run this exact command with --message-id {message_id} to replay it",
                printable(intent)
            ),
            "control" | "correction" => bail!(
                "The Codewhale service reported a {} but returned no Work record, so nothing is confirmed. Run `codewhale account agents work-status` on the Work you meant",
                printable(intent)
            ),
            _ => {}
        }
    }
    writeln!(out, "Intent: {}", printable(intent))?;
    if !receipt.reason.is_empty() {
        writeln!(out, "Reason: {}", printable(&receipt.reason))?;
    }
    if receipt.confident == Some(false) {
        writeln!(out, "The Agent was not confident in this reading.")?;
    }
    match intent {
        "control" => {
            let action = receipt
                .control_action
                .as_deref()
                .map(printable)
                .filter(|action| !action.is_empty())
                .unwrap_or_else(|| "unspecified".to_string());
            writeln!(out, "Applied control action: {action}")?;
        }
        "correction" => writeln!(
            out,
            "Applied: this message edited the objective of active Work. No new Work was created."
        )?,
        "queued" => writeln!(out, "This Work was queued behind active Work.")?,
        "actionable" => {}
        "informational" => {
            writeln!(out, "No Work was created and nothing was changed.")?;
            if let Some(suggestion) = receipt
                .suggestion
                .as_deref()
                .map(printable)
                .filter(|suggestion| !suggestion.is_empty())
            {
                writeln!(out, "Suggestion: {suggestion}")?;
                writeln!(
                    out,
                    "To have the Agent do it, send it as an instruction that starts with a verb such as fix, add or update."
                )?;
            }
        }
        _ => writeln!(
            out,
            "The Agent reported an intent this CLI does not recognize; verify with `codewhale account agents work-status`."
        )?,
    }
    for item in &items {
        write_work_item(out, item)?;
    }
    match (intent, items.first()) {
        ("actionable" | "queued", Some(first)) => {
            writeln!(
                out,
                "Work is recorded, not started. To run it on bounded Boat trial compute, review a quote first:"
            )?;
            writeln!(
                out,
                "  codewhale account agents work-quote {} --operation-key <new-key>",
                first.id
            )?;
        }
        ("control" | "correction", _) => writeln!(
            out,
            "Status above is the Work's state after this message; `work-status` confirms it."
        )?,
        _ => {}
    }
    writeln!(out, "Message ID: {message_id}")?;
    Ok(())
}

/// `codewhale account agents work-cancel`.
pub(super) fn cancel<T: CloudTransport, W: Write>(
    client: &CloudClient<'_, T>,
    out: &mut W,
    id: &str,
    queue: Option<WorkQueueChoice>,
    reason: Option<&str>,
) -> Result<()> {
    let id = validate_resource_id(id, "Work")?;
    let mut body = json!({});
    if let Some(reason) = reason {
        body["reason"] = json!(validate_named_text(
            reason,
            "Cancel reason",
            MAX_CANCEL_REASON_CHARS
        )?);
    }
    if let Some(queue) = queue {
        body["queue"] = json!(match queue {
            WorkQueueChoice::Discard => "discard",
            WorkQueueChoice::Park => "park",
        });
    }
    let unknown = |err: anyhow::Error| {
        err.context(format!(
            "The cancel outcome is unknown: the request may or may not have been applied. Run `codewhale account agents work-status {id}` before retrying; cancelling is safe to repeat"
        ))
    };
    let response = client
        .execute_authenticated(
            HttpMethod::Post,
            &format!("/api/runs/{id}/cancel"),
            Some(json_body(&body)?),
        )
        .map_err(|err| {
            if outcome_unknown(&err) {
                unknown(err)
            } else {
                err
            }
        })?;
    if !(200..300).contains(&response.status) {
        let err = response_error(&response);
        return match (response.status, http_code(&err)) {
            (409, Some("run_control_terminal")) => {
                writeln!(out, "Work ID: {id}")?;
                writeln!(out, "Work is already final; there is nothing to cancel.")?;
                writeln!(
                    out,
                    "Read its outcome: codewhale account agents work-result {id}"
                )?;
                Ok(())
            }
            (422, Some("run_prompt_queue_choice_required")) => Err(err.context(
                "This Work still has queued prompts and was not cancelled. Re-run with --queue discard or --queue park to choose what happens to them",
            )),
            (404, _) => Err(err.context(format!("Work {id} was not found on this account"))),
            _ if outcome_unknown(&err) => Err(unknown(err)),
            _ => Err(err.context("The cancel request was refused")),
        };
    }
    let reply = parse_reply(
        &response.body,
        "The Codewhale service returned an unreadable cancel reply",
    )
    .map_err(unknown)?;
    if let Some(run_id) = str_at(&reply, &["run", "id"])
        && run_id != id
    {
        bail!("The Codewhale service returned a different Work record");
    }
    writeln!(out, "Work ID: {id}")?;
    let state = text_at(&reply, &["run", "state"]);
    if reply.get("queued").is_some() && state.is_none() {
        writeln!(
            out,
            "Cancel requested. The runtime has not confirmed the stop yet."
        )?;
        if reply.get("replayed").and_then(Value::as_bool) == Some(true) {
            writeln!(out, "This cancellation was already pending.")?;
        }
        writeln!(
            out,
            "Check `codewhale account agents work-status {id}` until its status is canceled."
        )?;
    } else if let Some(state) = state {
        writeln!(out, "Status: {state}")?;
        if state == "canceled" {
            writeln!(out, "Work canceled.")?;
        } else {
            writeln!(
                out,
                "Cancel accepted; run `codewhale account agents work-status {id}` to confirm the final state."
            )?;
        }
    } else {
        writeln!(
            out,
            "Cancel accepted; run `codewhale account agents work-status {id}` to confirm the final state."
        )?;
    }
    if reply.get("promptQueue").is_some_and(Value::is_object)
        && let Some(queue) = queue
    {
        writeln!(
            out,
            "Queued prompts: {} (recorded).",
            match queue {
                WorkQueueChoice::Discard => "discarded",
                WorkQueueChoice::Park => "parked",
            }
        )?;
    }
    writeln!(
        out,
        "Compute stop and provider usage are final only in `codewhale account agents work-result {id}`."
    )?;
    Ok(())
}

/// The model route recorded for a Work, wherever the account API reports it.
fn recorded_model_route(envelope: &Value, result: &Value) -> Option<(String, String)> {
    let candidates = [
        at(result, &["modelRoute"]),
        at(result, &["modelAuthority"]),
        at(result, &["model"]),
        at(result, &["run", "modelAuthority"]),
        at(envelope, &["modelAuthority"]),
        at(result, &["run"]),
    ];
    candidates.into_iter().flatten().find_map(|candidate| {
        let provider =
            str_at(candidate, &["provider"]).or_else(|| str_at(candidate, &["modelProvider"]))?;
        let model = str_at(candidate, &["model"])?;
        validate_model_route(provider, model)
            .ok()
            .map(|(provider, model)| (provider.to_string(), model.to_string()))
    })
}

fn usd(value: f64) -> String {
    if value == 0.0 {
        "$0".to_string()
    } else {
        format!("${value:.4}")
    }
}

fn write_boat_usage<W: Write>(out: &mut W, envelope: &Value, result: &Value) -> Result<()> {
    let block = [
        at(result, &["boatUsage"]),
        at(result, &["providerUsage"]),
        at(result, &["usage"]),
        at(envelope, &["boatUsage"]),
        at(envelope, &["providerUsage"]),
        at(envelope, &["usage"]),
    ]
    .into_iter()
    .flatten()
    .find(|block| block.is_object());
    let Some(block) = block else {
        return Ok(());
    };
    writeln!(out, "Boat usage:")?;
    let number = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| block.get(*key).and_then(Value::as_f64))
    };
    if let Some(seconds) = number(&["providerSeconds", "seconds", "usedSeconds"]) {
        writeln!(out, "  Provider seconds: {seconds}")?;
    }
    if let Some(dollars) = number(&["providerListPriceDollars", "dollars"]) {
        writeln!(out, "  Provider list price: {}", usd(dollars))?;
    }
    if let Some(charged) = number(&["customerCreditsChargedUsd", "customerChargeDollars"]) {
        writeln!(out, "  Codewhale credits charged: {}", usd(charged))?;
    }
    if let Some(funding) = text_at(block, &["funding"]) {
        writeln!(out, "  Funding: {funding}")?;
    }
    if let Some(running) = block.get("running").and_then(Value::as_bool) {
        writeln!(
            out,
            "  Provider VM: {}",
            if running { "still running" } else { "stopped" }
        )?;
    }
    if let Some(confirmed) = block.get("cleanupConfirmed").and_then(Value::as_bool) {
        writeln!(
            out,
            "  Provider cleanup: {}",
            if confirmed { "confirmed" } else { "pending" }
        )?;
    }
    Ok(())
}

fn write_work_result<W: Write>(
    out: &mut W,
    id: &str,
    envelope: &Value,
    attempts: Option<&Value>,
) -> Result<()> {
    let result = envelope
        .get("result")
        .filter(|result| result.is_object())
        .ok_or_else(|| anyhow!("The Codewhale service returned no Work result"))?;
    if str_at(result, &["run", "id"]) != Some(id) {
        bail!("The Codewhale service returned a different Work result");
    }
    writeln!(out, "Work ID: {id}")?;
    writeln!(
        out,
        "State: {}",
        text_at(result, &["run", "state"]).unwrap_or_else(|| "unknown".to_string())
    )?;
    let status = text_at(result, &["status"]);
    if let Some(status) = &status {
        writeln!(out, "Result: {status}")?;
    }
    if let Some(title) = text_at(result, &["run", "title"]) {
        writeln!(out, "Objective: {title}")?;
    }
    if status.as_deref() == Some("in_progress") {
        writeln!(out, "This Work is still running; nothing below is final.")?;
    }
    if let Some(summary) = str_at(result, &["summary", "text"])
        .map(|text| printable_max(text, 600))
        .filter(|text| !text.is_empty())
    {
        writeln!(out, "Summary: {summary}")?;
    }

    let files = count_at(result, &["changes", "fileCount"]).unwrap_or(0);
    match text_at(result, &["changes", "evidence"]).as_deref() {
        Some("recorded") => writeln!(
            out,
            "Changes: {files} file(s), +{} -{} (recorded)",
            count_at(result, &["changes", "additions"]).unwrap_or(0),
            count_at(result, &["changes", "deletions"]).unwrap_or(0)
        )?,
        Some(other) => writeln!(out, "Changes: not verified (evidence: {other})")?,
        None => writeln!(out, "Changes: not reported")?,
    }
    if let Some(checks) = at(result, &["checks"]).and_then(Value::as_array) {
        if checks.is_empty() {
            writeln!(out, "Checks: none recorded")?;
        } else {
            let passed = checks
                .iter()
                .filter(|check| check.get("passed").and_then(Value::as_bool) == Some(true))
                .count();
            writeln!(out, "Checks: {passed} passed of {}", checks.len())?;
            for check in checks
                .iter()
                .filter(|check| check.get("passed").and_then(Value::as_bool) != Some(true))
                .take(10)
            {
                writeln!(
                    out,
                    "  not passed: {} ({})",
                    text_at(check, &["name"]).unwrap_or_else(|| "check".to_string()),
                    text_at(check, &["status"]).unwrap_or_else(|| "unverified".to_string())
                )?;
            }
        }
    }
    if let Some(findings) = at(result, &["findings"]).and_then(Value::as_array)
        && !findings.is_empty()
    {
        writeln!(out, "Findings: {}", findings.len())?;
        for finding in findings.iter().take(5) {
            writeln!(
                out,
                "  {}: {}",
                text_at(finding, &["severity"]).unwrap_or_else(|| "info".to_string()),
                text_at(finding, &["title"]).unwrap_or_else(|| "finding".to_string())
            )?;
        }
    }
    if let Some(approvals) = at(result, &["pendingApprovals"]).and_then(Value::as_array)
        && !approvals.is_empty()
    {
        writeln!(out, "Waiting on {} approval(s).", approvals.len())?;
    }
    let artifacts = at(result, &["artifacts"])
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if artifacts.is_empty() {
        writeln!(out, "Artifacts: none")?;
    } else {
        writeln!(out, "Artifacts ({}):", artifacts.len())?;
        for artifact in artifacts.iter().take(20) {
            writeln!(
                out,
                "  {} ({}, {}, {} bytes)",
                text_at(artifact, &["name"]).unwrap_or_else(|| "artifact".to_string()),
                text_at(artifact, &["status"]).unwrap_or_else(|| "unknown".to_string()),
                text_at(artifact, &["contentType"]).unwrap_or_else(|| "unknown type".to_string()),
                count_at(artifact, &["size"]).unwrap_or(0)
            )?;
        }
    }

    let repository = str_at(result, &["repository", "name"]).unwrap_or_default();
    if let Some(branch) = text_at(result, &["repository", "branch"]) {
        writeln!(out, "Branch: {branch}")?;
    }
    if let Some(revision) = text_at(result, &["repository", "revision"]) {
        writeln!(out, "Revision: {revision}")?;
    }
    match str_at(result, &["repository", "pullRequest", "url"]) {
        Some(url) => match github_pull_request_url(url, repository) {
            Some(url) => {
                let draft =
                    str_at(result, &["repository", "pullRequest", "state"]) == Some("draft");
                writeln!(
                    out,
                    "{}: {url}",
                    if draft { "Draft PR" } else { "Pull request" }
                )?;
            }
            None => writeln!(
                out,
                "Pull request: the service reported a link that is not a GitHub pull request for this repository, so it is not shown"
            )?,
        },
        None => writeln!(out, "Draft PR: none")?,
    }
    if let Some(blockers) = at(result, &["readiness", "blockers"]).and_then(Value::as_array)
        && !blockers.is_empty()
    {
        writeln!(
            out,
            "Blockers: {}",
            blockers
                .iter()
                .filter_map(Value::as_str)
                .map(printable)
                .collect::<Vec<_>>()
                .join(", ")
        )?;
    }
    if let Some(next) = text_at(result, &["nextAction", "label"]) {
        writeln!(out, "Next: {next}")?;
    }
    match recorded_model_route(envelope, result) {
        Some((provider, model)) => writeln!(out, "Model route: {provider}/{model}")?,
        None => writeln!(out, "Model route: not reported by the account API")?,
    }
    write_boat_usage(out, envelope, result)?;
    match attempts {
        Some(attempts) => {
            let rows = at(attempts, &["attempts"])
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            writeln!(
                out,
                "Attempts: {}",
                count_at(attempts, &["attemptCount"]).unwrap_or(rows.len() as u64)
            )?;
            for attempt in rows.iter().take(10) {
                let mut line = format!(
                    "  #{} {} {}",
                    count_at(attempt, &["sequence"]).unwrap_or(0),
                    text_at(attempt, &["kind"]).unwrap_or_else(|| "attempt".to_string()),
                    text_at(attempt, &["status"]).unwrap_or_else(|| "unknown".to_string())
                );
                if let Some(code) = text_at(attempt, &["errorCode"]) {
                    line.push_str(&format!(" (error {code})"));
                }
                writeln!(out, "{line}")?;
            }
        }
        None => writeln!(out, "Attempts: unavailable")?,
    }
    if let Some(seq) = count_at(result, &["receipt", "eventsThroughSeq"]) {
        writeln!(out, "Evidence through event {seq}")?;
    }
    Ok(())
}

/// `codewhale account agents work-result`.
pub(super) fn result<T: CloudTransport, W: Write>(
    client: &CloudClient<'_, T>,
    out: &mut W,
    id: &str,
    json: bool,
) -> Result<()> {
    let id = validate_resource_id(id, "Work")?;
    let envelope: Value = expect_json(
        client.execute_authenticated(HttpMethod::Get, &format!("/api/runs/{id}/result"), None)?,
        &[200],
    )?;
    // Attempt lineage is supporting evidence: without it the result is still
    // worth showing, so its failure is reported instead of hiding the result.
    let attempts = client
        .execute_authenticated(HttpMethod::Get, &format!("/api/runs/{id}/attempts"), None)
        .and_then(|response| expect_json::<Value>(response, &[200]))
        .ok();
    if json {
        return write_computer_json(
            out,
            &json!({ "result": envelope, "attempts": attempts.unwrap_or(Value::Null) }),
        );
    }
    write_work_result(out, id, &envelope, attempts.as_ref())
}

struct LaunchTarget {
    run_id: String,
    agent_id: String,
    project_id: String,
    repo: String,
    prompt: String,
    state: String,
}

/// The Work as the account serves it: the launch acts on this record, not on
/// anything the caller types, so the reviewed quote and the launch agree.
fn load_launch_target<T: CloudTransport>(
    client: &CloudClient<'_, T>,
    id: &str,
) -> Result<LaunchTarget> {
    let envelope: Value = expect_json(
        client.execute_authenticated(HttpMethod::Get, &format!("/api/runs/{id}"), None)?,
        &[200],
    )?;
    let run = envelope
        .get("run")
        .filter(|run| run.is_object())
        .ok_or_else(|| anyhow!("The Codewhale service returned no Work record"))?;
    if str_at(run, &["id"]) != Some(id) {
        bail!("The Codewhale service returned a different Work record");
    }
    let field = |name: &str| str_at(run, &[name]).unwrap_or_default().trim().to_string();
    let provider = field("repoProvider").to_ascii_lowercase();
    if !provider.is_empty() && provider != "github" {
        bail!(
            "Boat trial Work supports GitHub repositories only; this Work uses {}",
            printable(&provider)
        );
    }
    let agent_id = field("agentId");
    let project_id = field("projectId");
    if agent_id.is_empty() || project_id.is_empty() {
        bail!(
            "This Work is not attached to an Agent and Project, so it cannot be launched; create it with `codewhale account agents work`"
        );
    }
    validate_resource_id(&agent_id, "Agent")?;
    validate_resource_id(&project_id, "Project")?;
    let prompt = field("title");
    if prompt.is_empty() || prompt.chars().count() > 32_000 {
        bail!("Work objective must contain 1-32000 characters");
    }
    Ok(LaunchTarget {
        run_id: id.to_string(),
        agent_id,
        project_id,
        repo: field("repo"),
        prompt,
        state: field("state"),
    })
}

/// Cross-check the Work against the Agent and Project the account serves now.
fn verify_launch_authority<T: CloudTransport>(
    client: &CloudClient<'_, T>,
    target: &mut LaunchTarget,
) -> Result<()> {
    let agents: AgentListResponse = serde_json::from_value(client.agents()?)
        .context("The Codewhale service returned an invalid Agent list")?;
    let agent = resolve_account_agent(&agents.agents, &target.agent_id)?;
    if agent.project_id != target.project_id {
        bail!(
            "Agent {} is no longer bound to this Work's Project; bind it again or create new Work",
            printable(&agent.name)
        );
    }
    let projects: ProjectListResponse = serde_json::from_value(client.projects()?)
        .context("The Codewhale service returned an invalid Project list")?;
    let project = projects
        .projects
        .iter()
        .find(|project| project.id == target.project_id)
        .ok_or_else(|| {
            anyhow!(
                "Project {} is not available on this account. Run `codewhale account projects list`",
                target.project_id
            )
        })?;
    if project.default_repo_provider != "github" || project.default_repo.trim().is_empty() {
        bail!(
            "Project {} has no GitHub repository, so its Work cannot be launched",
            printable(&project.name)
        );
    }
    if target.repo.is_empty() {
        target.repo = project.default_repo.trim().to_string();
    } else if !target
        .repo
        .eq_ignore_ascii_case(project.default_repo.trim())
    {
        bail!("This Work's repository differs from its Project's repository; create new Work");
    }
    Ok(())
}

/// Contract C5: the launch-quote and cloud-sessions request for one bounded
/// Boat trial. The quote and the launch send the same fields so the signed
/// confirmation binds to exactly what starts.
fn launch_body(target: &LaunchTarget, operation_key: &str) -> Result<Value> {
    Ok(json!({
        "workRunId": target.run_id,
        "agentId": target.agent_id,
        "projectId": target.project_id,
        "repo": validate_github_repo(&target.repo)?,
        "provider": "github",
        "prompt": target.prompt,
        "runnerKind": "hosted",
        "sandboxSku": BOAT_TRIAL_SKU,
        "estimatedSeconds": BOAT_TRIAL_SECONDS,
        "modelProvider": BOAT_TRIAL_MODEL_PROVIDER,
        "model": BOAT_TRIAL_MODEL,
        "billingMode": "byok_external",
        "computeRegion": "eu",
        "sandboxTargetRegion": "eu",
        "crossRegionSandboxOptIn": true,
        "operationKey": operation_key,
    }))
}

/// What to do next for a control-plane code the launch flow knows about.
fn launch_hint(code: &str) -> Option<&'static str> {
    Some(match code {
        "boat_work_trial_unavailable" => "Boat trial Work is not available for this account",
        "boat_work_provider_unavailable" => "Boat compute is not configured on this Codewhale API",
        "hosted_launch_quote_required"
        | "hosted_launch_quote_invalid"
        | "hosted_launch_quote_expired"
        | "hosted_launch_quote_mismatch" => {
            "The confirmation is not valid for this launch. Run `work-quote` again with the same --operation-key and use the new confirmation"
        }
        "launch_operation_mismatch" => {
            "This --operation-key belongs to different launch inputs. Check `work-status` and `work-result` before starting again; never replace the key to retry an unknown launch"
        }
        "work_run_not_queued" => {
            "Only queued Work can be launched. Run `work-status` to see what happened to it"
        }
        "repo_access_required" => {
            "Connect the repository first with `codewhale account github bind OWNER/REPO`"
        }
        "hosted_byok_required" => {
            "Save your DeepSeek key first with `codewhale account keys set deepseek`"
        }
        _ => return None,
    })
}

/// `codewhale account agents work-quote`.
pub(super) fn quote<T: CloudTransport, W: Write>(
    client: &CloudClient<'_, T>,
    out: &mut W,
    id: &str,
    operation_key: &str,
) -> Result<()> {
    let id = validate_work_uuid(id)?;
    let operation_key = validate_operation_key(operation_key)?;
    let mut target = load_launch_target(client, id)?;
    if target.state != "queued" {
        bail!(
            "Work is {}, and only queued Work can be quoted for launch. Run `codewhale account agents work-status {id}`",
            printable(&target.state)
        );
    }
    verify_launch_authority(client, &mut target)?;
    let body = launch_body(&target, operation_key)?;
    let response = client.execute_authenticated(
        HttpMethod::Post,
        "/api/sandbox/launch-quote",
        Some(json_body(&body)?),
    )?;
    if response.status != 200 {
        let err = response_error(&response);
        return Err(match http_code(&err).and_then(launch_hint) {
            Some(hint) => err.context(hint),
            None => err,
        });
    }
    let quote: Value = parse_json_body(&response.body)?;

    // This command starts only the $0 EU Boat trial. If the service quotes
    // anything else, refuse to hand out a confirmation for it.
    let seconds = count_at(&quote, &["disclosure", "computerTime", "estimatedSeconds"]);
    let expected = at(&quote, &["disclosure", "funding"]).and_then(Value::as_str)
        == Some("provider_trial")
        && at(&quote, &["disclosure", "customerCreditsChargedUsd"]).and_then(Value::as_f64)
            == Some(0.0)
        && str_at(&quote, &["quote", "sku"]) == Some(BOAT_TRIAL_SKU)
        && str_at(&quote, &["quote", "adapter"]) == Some("boat")
        && str_at(&quote, &["disclosure", "sandboxTargetRegion"]) == Some("eu")
        && str_at(&quote, &["disclosure", "modelInference", "billing"]) == Some("byok_external")
        && seconds == Some(BOAT_TRIAL_SECONDS);
    if !expected {
        bail!(
            "The Codewhale service quoted something other than the $0 five-minute EU Boat trial. Refusing to print a confirmation for it; nothing started"
        );
    }
    let token = str_at(&quote, &["confirmation", "token"]).unwrap_or_default();
    if !valid_confirmation(token)
        || str_at(&quote, &["confirmation", "workRunId"]).is_some_and(|quoted| quoted != id)
    {
        bail!("The Codewhale service returned an unusable launch confirmation");
    }
    let seconds = BOAT_TRIAL_SECONDS;
    writeln!(
        out,
        "Boat trial Work quote. Nothing has started and nothing is charged."
    )?;
    writeln!(out, "Work ID: {id}")?;
    writeln!(out, "Repository: {}", printable(&target.repo))?;
    writeln!(
        out,
        "Model: {BOAT_TRIAL_MODEL_PROVIDER}/{BOAT_TRIAL_MODEL} with your own DeepSeek key; DeepSeek bills your BYOK usage directly"
    )?;
    writeln!(
        out,
        "Computer: {BOAT_TRIAL_SKU} on Boat in the EU, up to {seconds} seconds"
    )?;
    writeln!(
        out,
        "Funding: provider trial; Codewhale credits charged: $0"
    )?;
    if let Some(estimate) =
        at(&quote, &["disclosure", "providerCostEstimateUsd"]).and_then(Value::as_f64)
    {
        writeln!(out, "Provider cost estimate: {}", usd(estimate))?;
    }
    writeln!(
        out,
        "EU placement: Codewhale admission attestation; Boat reports no region field. Repository code and Work files are admitted to EU compute."
    )?;
    if let Some(title) = text_at(&quote, &["confirmCopy", "title"]) {
        writeln!(out, "{title}")?;
    }
    if let Some(copy) = str_at(&quote, &["confirmCopy", "body"])
        .map(|body| printable_max(body, 800))
        .filter(|body| !body.is_empty())
    {
        writeln!(out, "{copy}")?;
    }
    if let Some(expires) = text_at(&quote, &["confirmation", "expiresAt"]) {
        writeln!(out, "Confirmation expires: {expires}")?;
    }
    writeln!(out, "Confirmation: {token}")?;
    writeln!(out, "Operation key: {operation_key}")?;
    writeln!(
        out,
        "To start: codewhale account agents work-launch {id} --operation-key {operation_key} --confirmation {token} --confirm-eu-compute"
    )?;
    Ok(())
}

/// `codewhale account agents work-launch`.
pub(super) fn launch<T: CloudTransport, W: Write>(
    client: &CloudClient<'_, T>,
    out: &mut W,
    id: &str,
    operation_key: &str,
    confirmation: &str,
    confirm_eu_compute: bool,
) -> Result<()> {
    if !confirm_eu_compute {
        bail!(
            "Boat trial Work runs your repository code and Work files on Boat's EU compute. Nothing was sent; re-run with --confirm-eu-compute to agree"
        );
    }
    let id = validate_work_uuid(id)?;
    let operation_key = validate_operation_key(operation_key)?;
    let confirmation = confirmation.trim();
    if !valid_confirmation(confirmation) {
        bail!("Confirmation must be the value printed by `codewhale account agents work-quote`");
    }
    let mut target = load_launch_target(client, id)?;
    // A queued Work is checked against the Agent and Project as they are now.
    // Any other state is a replay of a launch that already ran (or a Work that
    // cannot launch); the control plane's operation ledger decides which, so
    // the same command stays safe to repeat after a lost reply.
    if target.state == "queued" {
        verify_launch_authority(client, &mut target)?;
    }
    let mut body = launch_body(&target, operation_key)?;
    body["launchQuoteConfirmation"] = json!(confirmation);
    body["customerEuPlacementConsent"] = json!(true);

    let refused = |err: anyhow::Error| -> anyhow::Error {
        if http_code(&err) == Some("boat_task_replay_expired") {
            return err.context(format!(
                "The launch outcome is unknown and the provider's safe replay window expired. Run `codewhale account agents work-status {id}` and `work-result {id}`; operator reconciliation is required. Do not submit a new operation key or retry the provider allocation"
            ));
        }
        if outcome_unknown(&err) {
            return err.context(format!(
                "The launch outcome is unknown, and a computer may already be running. Run `codewhale account agents work-status {id}` (and `work-cancel {id}` to stop it). To retry, re-run this exact command with the same --operation-key and --confirmation; it replays the launch and cannot start a second computer"
            ));
        }
        match http_code(&err) {
            Some("launch_in_progress" | "launch_idempotency_commit_failed") => err.context(
                "This launch is still being recorded. Re-run this exact command with the same --operation-key and --confirmation",
            ),
            Some(code) => match launch_hint(code) {
                Some(hint) => err.context(hint),
                None => err.context("The launch was refused"),
            },
            None => err,
        }
    };
    let response = client
        .execute_authenticated(
            HttpMethod::Post,
            "/api/cloud-sessions",
            Some(json_body(&body)?),
        )
        .map_err(refused)?;
    if !matches!(response.status, 200..=202) {
        return Err(refused(response_error(&response)));
    }
    let reply = parse_reply(
        &response.body,
        "The Codewhale service returned an unreadable launch reply",
    )
    .map_err(refused)?;
    // The service acted, so a reply without its `session` document is an
    // unknown outcome, not a success and not a different Work.
    let Some(session) = reply.get("session").filter(|value| value.is_object()) else {
        return Err(refused(
            CloudTransportError::new(
                "The Codewhale service returned a launch reply without a session",
                std::io::Error::other("missing session"),
            )
            .into(),
        ));
    };
    let Some(returned_run_id) = str_at(session, &["run", "id"]) else {
        return Err(refused(
            CloudTransportError::new(
                "The Codewhale service returned a launch reply without a Work ID",
                std::io::Error::other("missing Work ID"),
            )
            .into(),
        ));
    };
    if returned_run_id != id {
        bail!(
            "The Codewhale service launched a different Work than requested. Run `codewhale account agents work-status {id}` before doing anything else"
        );
    }
    writeln!(out, "Work launched on bounded Boat trial compute.")?;
    writeln!(out, "Work ID: {id}")?;
    writeln!(
        out,
        "Status: {}",
        text_at(session, &["run", "state"]).unwrap_or_else(|| "unknown".to_string())
    )?;
    if let Some(session_id) = text_at(session, &["id"]) {
        writeln!(out, "Session: {session_id}")?;
    }
    if let Some(provider) = text_at(session, &["sandbox", "provider"]) {
        writeln!(
            out,
            "Computer: {provider} ({})",
            text_at(session, &["sandbox", "status"])
                .unwrap_or_else(|| "status unknown".to_string())
        )?;
    }
    if let Some(region) = text_at(session, &["sandboxTargetRegion"]) {
        writeln!(out, "Compute region: {region}")?;
    }
    if let Some(charged) =
        at(session, &["quote", "customerCreditsChargedUsd"]).and_then(Value::as_f64)
    {
        writeln!(out, "Codewhale credits charged: {}", usd(charged))?;
    }
    if let Some(attempt) = text_at(session, &["attempt", "status"]) {
        writeln!(out, "Attempt: {attempt}")?;
    }
    if let Some(turn) = text_at(session, &["initialTurn", "status"]) {
        writeln!(out, "First turn: {turn} (not complete yet)")?;
    }
    writeln!(out, "Operation key: {operation_key}")?;
    writeln!(
        out,
        "Follow it: codewhale account agents work-status {id}, then work-result {id}. Stop it: work-cancel {id}"
    )?;
    Ok(())
}

/// `codewhale account github bind`.
pub(super) fn bind_github_repo<T: CloudTransport, W: Write>(
    client: &CloudClient<'_, T>,
    out: &mut W,
    repo: &str,
    installation_id: Option<&str>,
) -> Result<()> {
    let repo = validate_github_repo(repo)?;
    let installation_id = installation_id
        .map(|value| {
            let value = value.trim();
            if value.is_empty()
                || value.len() > 20
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                bail!("GitHub installation ID must be the numeric ID of the installed app");
            }
            Ok(value.to_string())
        })
        .transpose()?;
    let listing: GitHubBindingListResponse = serde_json::from_value(client.github_bindings()?)
        .context("The Codewhale service returned an invalid GitHub repository list")?;
    let usable = |status: &str| {
        !["error", "revoked", "suspended", "disabled"]
            .contains(&status.to_ascii_lowercase().as_str())
    };
    if let Some(existing) = listing.bindings.iter().find(|binding| {
        binding.provider == "github"
            && binding.repo.eq_ignore_ascii_case(&repo)
            && !binding.installation_id.is_empty()
            && usable(&binding.status)
            && installation_id
                .as_deref()
                .is_none_or(|requested| requested == binding.installation_id)
    }) {
        writeln!(
            out,
            "{} is already connected: {} ({})",
            printable(&existing.repo),
            printable(&existing.id),
            printable(&existing.status)
        )?;
        writeln!(
            out,
            "Create a Project: codewhale account projects create NAME --repo-binding-id {} --operation-key <new-key>",
            printable(&existing.id)
        )?;
        return Ok(());
    }
    let installation_id = match installation_id {
        Some(id) => id,
        None => {
            let mut known = listing
                .bindings
                .iter()
                .filter(|binding| binding.provider == "github")
                .map(|binding| binding.installation_id.as_str())
                .filter(|id| !id.is_empty())
                .collect::<Vec<_>>();
            known.sort_unstable();
            known.dedup();
            match known.as_slice() {
                [only] => (*only).to_string(),
                [] => bail!(
                    "No GitHub App installation is known for this account yet. Install the Codewhale GitHub App, then pass its numeric ID with --installation-id"
                ),
                many => bail!(
                    "This account has several GitHub App installations ({}); choose one with --installation-id",
                    many.iter()
                        .map(|id| printable(id))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        }
    };
    let response = client
        .execute_authenticated(
            HttpMethod::Post,
            "/api/integrations/github/bindings",
            Some(json_body(&json!({
                "installationId": installation_id,
                "repo": repo,
            }))?),
        )
        .map_err(|err| {
            if outcome_unknown(&err) {
                err.context(
                    "The bind outcome is unknown. Run `codewhale account github bindings` to see whether it was saved; binding is safe to repeat",
                )
            } else {
                err
            }
        })?;
    if response.status == 404 {
        return Err(response_error(&response)).context(
            "GitHub repository bindings are unavailable on this Codewhale API, or this repository is not reachable through that installation",
        );
    }
    if !matches!(response.status, 200 | 201) {
        let err = response_error(&response);
        return Err(if outcome_unknown(&err) {
            err.context(
                "The bind outcome is unknown. Run `codewhale account github bindings` to see whether it was saved; binding is safe to repeat",
            )
        } else {
            err
        });
    }
    let reply = parse_reply(&response.body, "The Codewhale service returned an unreadable bind reply")
        .map_err(|err| {
            err.context(
                "The bind outcome is unknown. Run `codewhale account github bindings` to see whether it was saved",
            )
        })?;
    let binding: AccountGitHubBinding =
        serde_json::from_value(reply.get("binding").cloned().unwrap_or(Value::Null))
            .context("The Codewhale service returned an invalid GitHub repository binding")?;
    if binding.provider != "github"
        || binding.id.is_empty()
        || !binding.repo.eq_ignore_ascii_case(&repo)
    {
        bail!("The Codewhale service returned a binding for a different repository");
    }
    writeln!(
        out,
        "Connected {}: {} ({})",
        printable(&binding.repo),
        printable(&binding.id),
        printable(&binding.status)
    )?;
    writeln!(
        out,
        "Create a Project: codewhale account projects create NAME --repo-binding-id {} --operation-key <new-key>",
        printable(&binding.id)
    )?;
    Ok(())
}
