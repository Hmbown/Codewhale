//! Populate rust-i18n from static entries with a bounded initialization stack.

use std::borrow::Cow;

type Messages = &'static [(&'static str, &'static str)];
include!(concat!(env!("OUT_DIR"), "/i18n_data.rs"));

pub(super) fn new() -> rust_i18n::SimpleBackend {
    LOCALES
        .iter()
        .map(|(locale, entries)| {
            (
                Cow::Borrowed(*locale),
                entries
                    .iter()
                    .map(|(key, value)| (Cow::Borrowed(*key), Cow::Borrowed(*value)))
                    .collect(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_i18n::Backend;

    #[test]
    fn every_translation_initializes_and_matches_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                // Construct a fresh backend even if another test already
                // initialized the global one. This catches catalog growth
                // reintroducing an oversized initialization frame.
                let backend = new();
                assert_eq!(backend.available_locales().len(), LOCALES.len());
                for (locale, entries) in LOCALES {
                    for (key, value) in *entries {
                        assert_eq!(backend.translate(locale, key).as_deref(), Some(*value));
                    }
                }
            })
            .expect("spawn bounded-stack translation test")
            .join()
            .expect("translation initialization must not overflow");
    }
}
