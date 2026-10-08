use codewhale_config::catalog::configured::{ConfiguredModel, validate_configured_models};
use codewhale_config::route::{
    CapabilityState, LogicalModelRef, OverrideSource, RouteRequest, RouteResolver,
};
use codewhale_config::{ConfigStore, ConfigToml, ProviderKind};

const FIXTURE: &str = include_str!("fixtures/custom_models.toml");
const ID: &str = "deepseek-v4.1-flash-expires-on-0910";
const BASE: &str = "https://models.example.test/v1";

fn models() -> Vec<ConfiguredModel> {
    toml::from_str::<ConfigToml>(FIXTURE)
        .unwrap()
        .custom_models
        .unwrap()
}

fn request(base: &str, id: &str) -> RouteRequest {
    RouteRequest {
        explicit_provider: Some(ProviderKind::Deepseek),
        model_selector: Some(LogicalModelRef::from(id)),
        base_url_override: Some(base.into()),
        ..RouteRequest::default()
    }
}

#[test]
fn persisted_model_roundtrip_reload_drives_immutable_route() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, FIXTURE).unwrap();
    let mut store = ConfigStore::load(Some(path.clone())).unwrap();
    let original = store.config.custom_models.clone();
    store.config.telemetry = Some(true);
    store.save().unwrap();
    let mut reloaded = ConfigStore::load(Some(path)).unwrap();
    assert_eq!(reloaded.config.custom_models, original);
    let definitions = reloaded.config.custom_models.as_ref().unwrap();
    assert_eq!(
        definitions[0].display_name.as_deref(),
        Some("Temporary preview")
    );
    assert_eq!(
        definitions[0].extras["future_note"].as_str(),
        Some("preserve this metadata")
    );
    let resolver = RouteResolver::new().with_configured_models(
        definitions,
        "deepseek",
        ProviderKind::Deepseek,
        BASE,
    );
    let old = resolver.resolve(&request(BASE, ID)).unwrap();
    assert_eq!(old.wire_model_id().as_str(), ID);
    assert_eq!(old.limits().context_tokens, Some(96000));
    assert_eq!(old.limits().input_tokens, Some(88000));
    assert_eq!(old.limits().output_tokens, Some(8000));
    assert_eq!(old.capabilities().image_input, CapabilityState::Unknown);
    assert_eq!(
        old.capabilities().native_tool_calls,
        CapabilityState::Unknown
    );
    assert_eq!(
        old.capabilities().structured_output,
        CapabilityState::Unsupported
    );
    assert!(
        old.applied_limit_overrides()
            .iter()
            .all(|entry| entry.source == OverrideSource::UserModelMetadata)
    );
    reloaded.config.custom_models.as_mut().unwrap()[0]
        .limit
        .as_mut()
        .unwrap()
        .context = Some(128000);
    reloaded.save().unwrap();
    reloaded.reload().unwrap();
    let next = RouteResolver::new()
        .with_configured_models(
            reloaded.config.custom_models.as_ref().unwrap(),
            "deepseek",
            ProviderKind::Deepseek,
            BASE,
        )
        .resolve(&request(BASE, ID))
        .unwrap();
    assert_eq!(next.limits().context_tokens, Some(128000));
    assert_eq!(old.limits().context_tokens, Some(96000));
}

#[test]
fn exact_identity_and_endpoint_do_not_leak_declarations() {
    let resolver = RouteResolver::new().with_configured_models(
        &models(),
        "deepseek",
        ProviderKind::Deepseek,
        BASE,
    );
    for base in [
        "https://other.example.test/v1",
        "http://models.example.test/v1",
        "https://models.example.test:444/v1",
        "https://models.example.test/V1",
    ] {
        let route = resolver.resolve(&request(base, ID)).unwrap();
        assert_eq!(route.limits().context_tokens, None, "{base}");
    }
    for id in ["deepseek-v4.1-flash", "DEEPSEEK-V4.1-FLASH-EXPIRES-ON-0910"] {
        assert_eq!(
            resolver
                .resolve(&request(BASE, id))
                .unwrap()
                .limits()
                .context_tokens,
            None,
            "{id}"
        );
    }
    assert_eq!(
        resolver
            .resolve(&request("https://MODELS.example.test:443/v1/", ID))
            .unwrap()
            .limits()
            .context_tokens,
        Some(96000)
    );
    let wrong_identity = RouteResolver::new().with_configured_models(
        &models(),
        "another-provider",
        ProviderKind::Deepseek,
        BASE,
    );
    assert_eq!(
        wrong_identity
            .resolve(&request(BASE, ID))
            .unwrap()
            .limits()
            .context_tokens,
        None
    );
}

#[test]
fn unknown_fields_are_not_filled_from_a_known_sibling() {
    let mut definitions = models();
    definitions[0].limit = None;
    definitions[0].cost = None;
    definitions[0].reasoning = None;
    definitions[0].modalities = None;
    definitions[0].tool_call = None;
    let offering = definitions[0].to_catalog_offering();
    assert!(offering.limit.is_none());
    assert!(offering.cost.is_none());
    assert!(offering.reasoning.is_none());
    let route = RouteResolver::new()
        .with_configured_models(&definitions, "deepseek", ProviderKind::Deepseek, BASE)
        .resolve(&request(BASE, ID))
        .unwrap();
    assert_eq!(route.limits().context_tokens, None);
    assert_eq!(route.capabilities().reasoning, CapabilityState::Unknown);
    assert_eq!(
        route.capabilities().native_tool_calls,
        CapabilityState::Unknown
    );
}

#[test]
fn declarations_cannot_expand_closed_protocol_rosters() {
    let mut definitions = models();
    definitions[0].provider = "opencode-zen".into();
    definitions[0].id = "unknown-protocol-model".into();
    let resolver = RouteResolver::new().with_configured_models(
        &definitions,
        "opencode-zen",
        ProviderKind::OpencodeZen,
        BASE,
    );
    let mut req = request(BASE, "unknown-protocol-model");
    req.explicit_provider = Some(ProviderKind::OpencodeZen);
    assert!(resolver.resolve(&req).is_err());
}

#[test]
fn invalid_limits_prices_units_and_authority_fail_closed() {
    for (before, after) in [
        ("context = 96000", "context = 0"),
        ("context = 96000", "context = 4294967296"),
        ("output = 8000", "output = 97000"),
        ("input = 0.4", "input = -1.0"),
        ("input = 0.4", "input = nan"),
        ("input = 0.4", "input = inf"),
        ("input = 0.4", "input = 0.4, currency = 'CNY'"),
        ("input = 0.4", "input = 0.4, unit = 'per_token'"),
        ("future_note =", "source ="),
        ("future_note =", "canonical_model ="),
        ("future_note =", "api_key ="),
        (ID, "auto"),
        (BASE, "https://user:password@models.example.test/v1"),
        (BASE, "https://models.example.test/v1?key=secret"),
    ] {
        assert!(
            toml::from_str::<ConfigToml>(&FIXTURE.replace(before, after)).is_err(),
            "{before} -> {after}"
        );
    }
    let mut duplicate = models();
    let mut other = duplicate[0].clone();
    other.base_url = "https://MODELS.example.test:443/v1/".into();
    duplicate.push(other);
    assert!(validate_configured_models(&duplicate).is_err());
}

#[test]
fn declared_wire_ids_are_not_convenience_aliases() {
    for (kind, provider, base, id) in [
        (
            ProviderKind::Together,
            "together",
            "https://api.together.xyz/v1",
            "inkling",
        ),
        (
            ProviderKind::Openrouter,
            "openrouter",
            "https://openrouter.ai/api/v1",
            "qwen3.7-plus",
        ),
        (
            ProviderKind::Concentrate,
            "concentrate",
            "https://api.concentrate.ai/v1",
            "concentrate/example",
        ),
        (
            ProviderKind::Deepseek,
            "deepseek",
            "https://api.deepseek.com",
            "deepseek-v4pro",
        ),
    ] {
        let mut definitions = models();
        definitions[0].provider = provider.into();
        definitions[0].base_url = base.into();
        definitions[0].id = id.into();
        let resolver =
            RouteResolver::new().with_configured_models(&definitions, provider, kind, base);
        let mut req = request(base, id);
        req.explicit_provider = Some(kind);
        let candidate = resolver.resolve(&req).unwrap();
        assert_eq!(candidate.wire_model_id().as_str(), id);
        assert_eq!(candidate.limits().output_tokens, Some(8000));
        req.base_url_override = Some("https://elsewhere.example.test/v1".into());
        assert_eq!(resolver.resolve(&req).unwrap().limits().output_tokens, None);
    }
}

#[test]
fn persisted_metadata_survives_unrelated_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, FIXTURE).unwrap();
    let mut store = ConfigStore::load(Some(path.clone())).unwrap();
    store.config.telemetry = Some(true);
    store.save().unwrap();
    let persisted: toml::Value = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        persisted
            .get("custom_models")
            .and_then(|models| models.as_array())
            .and_then(|models| models.first())
            .and_then(|model| model.get("id"))
            .and_then(|id| id.as_str()),
        Some(ID)
    );
}

#[test]
fn declared_wire_id_precedes_real_aggregator_canonical_aliases() {
    let id = "deepseek-v4-pro";
    for (provider, kind, base) in [
        (
            "openrouter",
            ProviderKind::Openrouter,
            "https://openrouter.ai/api/v1",
        ),
        (
            "together",
            ProviderKind::Together,
            "https://api.together.xyz/v1",
        ),
    ] {
        let mut definitions = models();
        definitions[0].provider = provider.into();
        definitions[0].base_url = base.into();
        definitions[0].id = id.into();
        let mut req = request(base, id);
        req.explicit_provider = Some(kind);
        let bundled = RouteResolver::new().resolve(&req).unwrap();
        assert_ne!(
            bundled.wire_model_id().as_str(),
            id,
            "fixture must hit real bundled alias"
        );
        let resolver =
            RouteResolver::new().with_configured_models(&definitions, provider, kind, base);
        for saved in [false, true] {
            if saved {
                req.model_selector = None;
                req.saved_provider_model = Some(id.into());
            }
            let candidate = resolver.resolve(&req).unwrap();
            assert_eq!(
                candidate.wire_model_id().as_str(),
                id,
                "{provider} saved={saved}"
            );
            assert!(candidate.canonical_model().is_none());
            assert_eq!(
                candidate.pricing(),
                Some(&codewhale_config::route::PricingSku::Token {
                    input_per_mtok: Some(0.4),
                    output_per_mtok: Some(1.6),
                })
            );
            assert_eq!(candidate.limits().context_tokens, Some(96000));
            assert_eq!(candidate.limits().output_tokens, Some(8000));
            assert!(
                candidate
                    .applied_limit_overrides()
                    .iter()
                    .all(|entry| entry.source == OverrideSource::UserModelMetadata)
            );
        }
        req.base_url_override = Some("https://unrelated.example.test/v1".into());
        let unrelated = resolver.resolve(&req).unwrap();
        assert_eq!(unrelated.limits().context_tokens, None);
        assert!(unrelated.applied_limit_overrides().is_empty());
    }
}

#[test]
fn all_positive_declared_capabilities_stay_unverified() {
    let mut definitions = models();
    for flag in [Some(true), Some(false), None] {
        let model = &mut definitions[0];
        model.reasoning = flag;
        model.tool_call = flag;
        model.attachment = flag;
        model.structured_output = flag;
        model.modalities =
            flag.map(
                |supported| codewhale_config::models_dev::ModelsDevModalities {
                    input: if supported {
                        vec!["text".into(), "image".into()]
                    } else {
                        vec!["text".into()]
                    },
                    output: vec!["text".into()],
                },
            );
        assert_eq!(model.to_catalog_offering().reasoning, flag);
        let candidate = RouteResolver::new()
            .with_configured_models(&definitions, "deepseek", ProviderKind::Deepseek, BASE)
            .resolve(&request(BASE, ID))
            .unwrap();
        let caps = candidate.capabilities();
        let expected = if flag == Some(false) {
            CapabilityState::Unsupported
        } else {
            CapabilityState::Unknown
        };
        for capability in [
            caps.reasoning,
            caps.image_input,
            caps.attachments,
            caps.native_tool_calls,
            caps.structured_output,
        ] {
            assert_eq!(capability, expected);
        }
    }
}
