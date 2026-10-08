//! `agent action=start runtime:"cloud"`: the model may propose a long job for
//! a cloud sandbox; only a person can start it.
//!
//! This is a thin front door onto `/dispatch`. It plans through
//! [`crate::cloud_dispatch::plan_dispatch`] and queues the job through
//! [`crate::cloud_dispatch::execute_dispatch`] with `confirm = false`, which
//! saves a `proposed` record and nothing else: no sandbox, no push, no spend.
//! The person confirms with `/dispatch confirm <id>`, and `confirm_job` runs
//! the same fail-closed credential and machine-token gates it always has.
//! Nothing here can confirm, so there is no auto-confirm to guard.

use std::path::Path;

use serde_json::{Value, json};

use crate::cloud_dispatch::{
    CloudJobStore, CredentialState, DispatchOutcome, Forge, GitRemote, MachineTokenState,
    discover_credentials, discover_machine_token, discover_remotes, execute_dispatch, format_job,
    missing_credentials_message, missing_machine_token_message, plan_dispatch,
};
use crate::tools::spec::{ToolError, ToolResult};

/// Where an `agent action=start` runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StartRuntime {
    Local,
    Cloud,
}

/// Fields a cloud proposal reads. Everything else on `start` shapes a local
/// child (scopes, roles, worktrees, limits); those are not refused, because a
/// retry costs more than they are worth, but the result names them as unused
/// so neither the model nor the person assumes they applied.
const CLOUD_START_FIELDS: &[&str] = &["action", "op", "runtime", "prompt", "remote"];

pub(super) fn parse_start_runtime(input: &Value) -> Result<StartRuntime, ToolError> {
    match input.get("runtime") {
        None | Some(Value::Null) => Ok(StartRuntime::Local),
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "" | "local" => Ok(StartRuntime::Local),
            "cloud" => Ok(StartRuntime::Cloud),
            other => Err(ToolError::invalid_input(format!(
                "runtime must be local or cloud, not `{other}`."
            ))),
        },
        Some(_) => Err(ToolError::invalid_input(
            "runtime must be a string: local or cloud.",
        )),
    }
}

pub(super) fn is_cloud_start(input: &Value) -> bool {
    matches!(parse_start_runtime(input), Ok(StartRuntime::Cloud))
}

/// Queue a cloud proposal for the person to confirm. Never spawns.
pub(super) fn propose_cloud_run(
    input: &Value,
    workspace: &Path,
    spawn_depth: u32,
) -> Result<ToolResult, ToolError> {
    if spawn_depth > 0 {
        return Err(ToolError::permission_denied(
            "Only the main session can propose a cloud run. Say in your report that this work needs one.",
        ));
    }
    let store = CloudJobStore::from_env()
        .map_err(|error| ToolError::execution_failed(error.to_string()))?;
    let payload = propose_with(
        input,
        &discover_remotes(workspace),
        &store,
        &discover_credentials(),
        &discover_machine_token(),
    )?;
    let mut result =
        ToolResult::json(&payload).map_err(|e| ToolError::execution_failed(e.to_string()))?;
    result.metadata = Some(json!({
        "action": "start",
        "runtime": "cloud",
        "status": "proposed",
        "job_id": payload["job_id"],
    }));
    Ok(result)
}

fn propose_with(
    input: &Value,
    remotes: &[GitRemote],
    store: &CloudJobStore,
    credentials: &CredentialState,
    machine_token: &MachineTokenState,
) -> Result<Value, ToolError> {
    let mut unused: Vec<&str> = input
        .as_object()
        .map(|object| {
            object
                .keys()
                .map(String::as_str)
                .filter(|key| !CLOUD_START_FIELDS.contains(key))
                .collect()
        })
        .unwrap_or_default();
    unused.sort_unstable();
    let prompt = match input.get("prompt") {
        Some(Value::String(prompt)) => prompt.as_str(),
        _ => {
            return Err(ToolError::invalid_input(
                "runtime=\"cloud\" needs prompt: the task the cloud agent should do.",
            ));
        }
    };
    let remote = match input.get("remote") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(
            Forge::parse(value)
                .ok_or_else(|| ToolError::invalid_input("remote must be github, cnb, or gitee."))?,
        ),
        Some(_) => {
            return Err(ToolError::invalid_input(
                "remote must be a string: github, cnb, or gitee.",
            ));
        }
    };
    let plan = plan_dispatch(remotes, prompt, remote, None)
        .map_err(|error| ToolError::invalid_input(error.to_string()))?;
    // `confirm = false` saves a proposal and returns before any credential,
    // sandbox or forge call. Confirmation belongs to the person.
    let job = match execute_dispatch(store, plan, false, credentials, machine_token)
        .map_err(|error| ToolError::execution_failed(error.to_string()))?
    {
        DispatchOutcome::Proposal(job) => job,
        DispatchOutcome::Refused(job) | DispatchOutcome::Accepted(job) => {
            return Err(ToolError::execution_failed(format!(
                "cloud job {} did not come back as a proposal; nothing was started from here.",
                job.id
            )));
        }
    };
    // Say now what confirm will refuse, so the person is not asked to
    // approve a job that cannot run.
    let blocked_by = match (credentials, machine_token) {
        (CredentialState::Missing, _) => Some(missing_credentials_message()),
        (_, MachineTokenState::Missing) => Some(missing_machine_token_message()),
        _ => None,
    };
    let mut summary = match &blocked_by {
        None => format!(
            "Proposed cloud job {id}. Nothing runs or spends until the person types \
             `/dispatch confirm {id}`. Do not confirm it yourself; tell the person it is waiting.",
            id = job.id
        ),
        Some(reason) => format!(
            "Proposed cloud job {id}, but confirming it will be refused until this is fixed: {reason}",
            id = job.id
        ),
    };
    if !unused.is_empty() {
        summary.push_str(&format!(
            " Not used by a cloud job, which gets only the prompt: {}.",
            unused.join(", ")
        ));
    }
    Ok(json!({
        "action": "start",
        "runtime": "cloud",
        "status": "proposed",
        "started": false,
        "job_id": job.id,
        "confirm_with": format!("/dispatch confirm {}", job.id),
        "cancel_with": format!("/dispatch cancel {}", job.id),
        "ready_to_confirm": blocked_by.is_none(),
        "blocked_by": blocked_by,
        "unused_fields": unused,
        "summary": summary,
        "card": format_job(&job),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_dispatch::{CloudJobStatus, CredentialSource};

    fn github_remote() -> Vec<GitRemote> {
        vec![GitRemote {
            name: "github".to_string(),
            url: "https://github.com/example/repo.git".to_string(),
        }]
    }

    const READY: CredentialState = CredentialState::Present {
        source: CredentialSource::Env,
    };

    #[test]
    fn runtime_defaults_to_local_and_rejects_unknown_values() {
        assert_eq!(
            parse_start_runtime(&json!({"prompt": "x"})).unwrap(),
            StartRuntime::Local
        );
        assert_eq!(
            parse_start_runtime(&json!({"runtime": "Cloud"})).unwrap(),
            StartRuntime::Cloud
        );
        assert!(parse_start_runtime(&json!({"runtime": "moon"})).is_err());
        assert!(parse_start_runtime(&json!({"runtime": true})).is_err());
        assert!(is_cloud_start(
            &json!({"action": "start", "runtime": "cloud"})
        ));
        assert!(!is_cloud_start(&json!({"action": "start"})));
    }

    #[test]
    fn cloud_start_only_queues_a_proposal_for_the_person() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = CloudJobStore::from_path(tmp.path().join("cloud-jobs"));
        let payload = propose_with(
            &json!({"action": "start", "runtime": "cloud", "prompt": "run the full soak suite and fix flakes"}),
            &github_remote(),
            &store,
            &READY,
            &MachineTokenState::Present,
        )
        .expect("proposal");
        assert_eq!(payload["status"], "proposed");
        assert_eq!(payload["started"], false);
        assert_eq!(payload["ready_to_confirm"], true);
        let id = payload["job_id"].as_str().expect("job id");
        assert_eq!(payload["confirm_with"], format!("/dispatch confirm {id}"));
        assert!(
            payload["summary"]
                .as_str()
                .unwrap()
                .contains("Do not confirm it yourself")
        );

        // The queued record is the same card `/dispatch show` renders, still
        // unconfirmed, with no sandbox.
        let jobs = store.list().expect("jobs");
        assert_eq!(jobs.len(), 1);
        let job = &jobs[0];
        assert_eq!(job.id, id);
        assert_eq!(job.status, CloudJobStatus::Proposed);
        assert!(!job.confirmed);
        assert!(job.sandbox_id.is_none());
        assert_eq!(payload["card"], format_job(job));
    }

    #[test]
    fn a_missing_machine_token_is_named_before_the_person_confirms() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = CloudJobStore::from_path(tmp.path().join("cloud-jobs"));
        let payload = propose_with(
            &json!({"runtime": "cloud", "prompt": "long job"}),
            &github_remote(),
            &store,
            &READY,
            &MachineTokenState::Missing,
        )
        .expect("proposal is still queued");
        assert_eq!(payload["ready_to_confirm"], false);
        assert_eq!(payload["blocked_by"], missing_machine_token_message());
        assert_eq!(
            store.list().expect("jobs")[0].status,
            CloudJobStatus::Proposed,
            "the gate runs at confirm, not here"
        );
    }

    #[test]
    fn local_only_fields_are_named_as_unused_not_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = CloudJobStore::from_path(tmp.path().join("cloud-jobs"));
        let payload = propose_with(
            &json!({"runtime": "cloud", "prompt": "x", "worktree": true, "type": "implement"}),
            &github_remote(),
            &store,
            &READY,
            &MachineTokenState::Present,
        )
        .expect("a proposal, not a retry");
        assert_eq!(payload["unused_fields"], json!(["type", "worktree"]));
        assert!(
            payload["summary"]
                .as_str()
                .unwrap()
                .contains("gets only the prompt: type, worktree."),
            "{payload}"
        );
        assert_eq!(store.list().expect("jobs").len(), 1);
    }

    #[test]
    fn bad_remotes_and_missing_prompts_are_refused_without_a_record() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = CloudJobStore::from_path(tmp.path().join("cloud-jobs"));
        let error = propose_with(
            &json!({"runtime": "cloud", "prompt": "x", "remote": "bitbucket"}),
            &github_remote(),
            &store,
            &READY,
            &MachineTokenState::Present,
        )
        .expect_err("unknown forge refused");
        assert!(
            error.to_string().contains("github, cnb, or gitee"),
            "{error}"
        );
        assert!(
            propose_with(
                &json!({"runtime": "cloud"}),
                &github_remote(),
                &store,
                &READY,
                &MachineTokenState::Present,
            )
            .is_err(),
            "prompt is required"
        );
        assert!(
            propose_with(
                &json!({"runtime": "cloud", "prompt": "x"}),
                &[],
                &store,
                &READY,
                &MachineTokenState::Present,
            )
            .is_err(),
            "no forge remote, no proposal"
        );
        assert!(store.list().expect("jobs").is_empty());
    }

    #[test]
    fn a_child_agent_cannot_propose() {
        let error = propose_cloud_run(
            &json!({"runtime": "cloud", "prompt": "x"}),
            Path::new("."),
            1,
        )
        .expect_err("children refused");
        assert!(error.to_string().contains("main session"), "{error}");
    }
}
