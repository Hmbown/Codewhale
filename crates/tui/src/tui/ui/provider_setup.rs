//! Provider-configuration support: runtime-preset file snapshots with
//! rollback, and the provider key verification seam
//! (TUI_MODULARIZATION.md slice 8).

use super::*;

pub(crate) trait ProviderKeyVerifier {
    fn verify<'a>(
        &'a self,
        provider: ProviderKind,
        api_key: &'a str,
        base_url: &'a str,
    ) -> ProviderKeyVerification<'a>;
}

pub(crate) struct LiveProviderKeyVerifier;

impl ProviderKeyVerifier for LiveProviderKeyVerifier {
    fn verify<'a>(
        &'a self,
        provider: ProviderKind,
        api_key: &'a str,
        base_url: &'a str,
    ) -> ProviderKeyVerification<'a> {
        Box::pin(crate::client::verify_provider_api_key(
            provider, api_key, base_url,
        ))
    }
}

pub(crate) struct RuntimePresetFileSnapshot {
    pub(crate) path: PathBuf,
    pub(crate) contents: Option<Vec<u8>>,
}

impl RuntimePresetFileSnapshot {
    pub(crate) fn capture(path: PathBuf) -> Result<Self> {
        let contents = match std::fs::read(&path) {
            Ok(contents) => Some(contents),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to snapshot {}", path.display()));
            }
        };
        Ok(Self { path, contents })
    }

    fn restore(&self) -> Result<()> {
        match &self.contents {
            Some(contents) => crate::utils::write_atomic(&self.path, contents)
                .with_context(|| format!("failed to restore {}", self.path.display())),
            None => match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => {
                    Err(error).with_context(|| format!("failed to remove {}", self.path.display()))
                }
            },
        }
    }
}

pub(crate) fn runtime_preset_error_with_rollback(
    error: anyhow::Error,
    snapshots: &[&RuntimePresetFileSnapshot],
) -> anyhow::Error {
    let rollback_errors = snapshots
        .iter()
        .filter_map(|snapshot| snapshot.restore().err())
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    if rollback_errors.is_empty() {
        error
    } else {
        anyhow::anyhow!(
            "{error:#}; runtime preset rollback also failed: {}",
            rollback_errors.join("; ")
        )
    }
}
