use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::{ApiError, RuntimeApiState, map_thread_err};

/// Live context-window posture for one thread — the facts the GPUI usage
/// panel cannot reconstruct from turn receipts. `input_tokens` is the same
/// conservative estimate the in-app context meter shows; `billed_input_tokens`
/// is the last provider-counted prompt size when one exists.
///
/// All numeric fields are nullable: a route that cannot express a bounded
/// window (unknown model, no catalog or configured limits) reports `null`
/// rather than an invented number, and `live: false` marks responses where
/// the engine could not be loaded and only the static route window resolved.
#[derive(Debug, Serialize)]
pub(super) struct ThreadContextResponse {
    thread_id: String,
    model: String,
    provider: Option<String>,
    model_provider_id: Option<String>,
    /// `true` when the numbers came from the loaded engine (live estimate +
    /// route limits); `false` when only the store-recorded route resolved.
    live: bool,
    window_tokens: Option<u64>,
    input_tokens: Option<u64>,
    billed_input_tokens: Option<u64>,
    output_cap_tokens: Option<u64>,
    input_budget_ceiling: Option<u64>,
    available_input_tokens: Option<u64>,
    compaction_trigger_tokens: Option<u64>,
    usage_percent: Option<f64>,
    pressure: Option<&'static str>,
}

pub(super) async fn get_thread_context(
    State(state): State<RuntimeApiState>,
    Path(thread_id): Path<String>,
) -> Result<Json<ThreadContextResponse>, ApiError> {
    let thread = state
        .runtime_threads
        .get_thread(&thread_id)
        .await
        .map_err(map_thread_err)?;

    if let Ok(engine) = state.runtime_threads.get_engine(&thread_id).await
        && let Ok(Some(snapshot)) = engine.get_context_budget().await
    {
        return Ok(Json(ThreadContextResponse {
            thread_id,
            model: snapshot.model,
            provider: Some(snapshot.provider),
            model_provider_id: snapshot.model_provider_id,
            live: true,
            window_tokens: Some(snapshot.window_tokens),
            input_tokens: Some(snapshot.input_tokens),
            billed_input_tokens: snapshot.billed_input_tokens,
            output_cap_tokens: Some(snapshot.output_cap_tokens),
            input_budget_ceiling: Some(snapshot.input_budget_ceiling),
            available_input_tokens: Some(snapshot.available_input_tokens),
            compaction_trigger_tokens: Some(snapshot.compaction_trigger_tokens),
            usage_percent: Some(snapshot.usage_percent),
            pressure: Some(snapshot.pressure),
        }));
    }

    // Engine unavailable or the route cannot bound a window: still answer
    // with whatever the thread record and the static route catalog can prove.
    let provider = thread
        .model_provider
        .as_deref()
        .and_then(crate::config::ApiProvider::parse);
    let window_tokens = provider.map(|provider| {
        u64::from(crate::route_budget::route_context_window_tokens(
            provider,
            &thread.model,
            None,
        ))
    });
    Ok(Json(ThreadContextResponse {
        thread_id,
        model: thread.model,
        provider: thread.model_provider,
        model_provider_id: thread.model_provider_id,
        live: false,
        window_tokens,
        input_tokens: None,
        billed_input_tokens: None,
        output_cap_tokens: None,
        input_budget_ceiling: None,
        available_input_tokens: None,
        compaction_trigger_tokens: None,
        usage_percent: None,
        pressure: None,
    }))
}
