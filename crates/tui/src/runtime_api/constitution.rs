use super::*;
use codewhale_config::user_constitution::ProfileConstitution;

pub(super) async fn get_constitution(
    State(state): State<RuntimeApiState>,
) -> Result<Json<Value>, ApiError> {
    let snapshot = crate::profile_constitution::load(state.config_profile.as_deref())
        .await
        .map_err(|error| ApiError::conflict(error.to_string()))?;
    match snapshot {
        Some(snapshot) => {
            let rendered = snapshot
                .render()
                .map_err(|error| ApiError::bad_request(error.to_string()))?;
            Ok(Json(
                json!({"source":"profile", "accountId":snapshot.account_id,
                "revision":snapshot.revision, "constitution":snapshot.constitution,
                "modelGuidance":rendered, "applies":"next_turn"}),
            ))
        }
        None => Ok(Json(json!({"source":"local", "revision":null,
            "modelGuidance":crate::prompts::load_user_constitution_block(), "applies":"next_turn"}))),
    }
}

pub(super) async fn preview_constitution(
    Json(document): Json<ProfileConstitution>,
) -> Result<Json<Value>, ApiError> {
    let rendered = document
        .as_user_constitution()
        .map_err(|error| ApiError::bad_request(error.to_string()))?
        .render_body();
    Ok(Json(json!({"modelGuidance":rendered, "saved":false})))
}
