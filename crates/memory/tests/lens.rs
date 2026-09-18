//! Native acceptance suite for the new lens. Requires a Rust toolchain.
use codewhale_memory::*;
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};
fn setup() -> (Store, Access, Scope, Scope) {
    let scope = Scope::user("local", "owner").workspace("repo");
    let session = scope.session("run");
    let access = Access::operator(vec![scope.clone(), session.clone()]).unwrap();
    (
        Store::in_memory_with_clock(|| 1000).unwrap(),
        access,
        scope,
        session,
    )
}
fn draft(scope: &Scope, body: &str) -> Draft {
    Draft::note(
        scope.clone(),
        "Whale",
        body,
        Evidence {
            kind: SourceKind::User,
            uri: "codewhale:input".into(),
            locator: "explicit".into(),
            sha256: None,
            observed_at: 900,
        },
    )
}
fn active(s: &mut Store, a: &Access, scope: &Scope, key: &str, body: &str) -> Memory {
    let c = s.capture(a, key, draft(scope, body)).unwrap().memory;
    s.approve(a, &c.id, c.revision, None, &Snapshot::default())
        .unwrap()
}
fn prepare(s: &Store, a: &Access, session: &Scope) -> lens::PreparedContext {
    s.prepare_context(
        a,
        session,
        "run",
        "request",
        &Recall::default(),
        &ByteCounter,
        &ContextBudget::default(),
    )
    .unwrap()
}
#[test]
fn lens_candidates_do_not_enter_a_context() {
    let (mut s, a, n, se) = setup();
    s.capture(&a, "c", draft(&n, "candidate")).unwrap();
    let p = prepare(&s, &a, &se);
    assert!(p.packet.selected.is_empty());
    assert_eq!(
        s.lens_snapshot(&a, Some("run"), None, 100, &Snapshot::default())
            .unwrap()
            .entries
            .len(),
        1
    );
}
#[test]
fn preparation_is_not_dispatch() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    let p = prepare(&s, &a, &se);
    assert_eq!(p.receipt.stage, "prepared");
    assert!(p.receipt.dispatched_at.is_none());
}
#[test]
fn delivery_requires_the_engine_capability() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    let p = prepare(&s, &a, &se);
    let model = Access::agent(vec![n, se]).unwrap();
    assert!(matches!(
        s.acknowledge_append(
            &model,
            &p.receipt.id,
            &p.receipt.packet_hash,
            1,
            &Snapshot::default()
        ),
        Err(Error::Denied)
    ));
}
#[test]
fn preparing_also_requires_the_engine_capability() {
    let (s, _, n, se) = setup();
    let model = Access::agent(vec![n, se.clone()]).unwrap();
    assert!(matches!(
        s.prepare_context(
            &model,
            &se,
            "run",
            "q",
            &Recall::default(),
            &ByteCounter,
            &ContextBudget::default()
        ),
        Err(Error::Denied)
    ));
}
#[test]
fn dispatch_cannot_skip_append() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    let p = prepare(&s, &a, &se);
    assert!(matches!(
        s.acknowledge_dispatch(
            &a,
            &p.receipt.id,
            &p.receipt.packet_hash,
            "transport",
            &Snapshot::default()
        ),
        Err(Error::InvalidState)
    ));
}
#[test]
fn packet_hash_is_checked() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    let p = prepare(&s, &a, &se);
    assert!(matches!(
        s.acknowledge_append(&a, &p.receipt.id, "wrong", 1, &Snapshot::default()),
        Err(Error::RevisionConflict)
    ));
}
#[test]
fn acknowledgement_is_idempotent() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    let p = prepare(&s, &a, &se);
    for _ in 0..2 {
        s.acknowledge_append(
            &a,
            &p.receipt.id,
            &p.receipt.packet_hash,
            1,
            &Snapshot::default(),
        )
        .unwrap();
        s.acknowledge_dispatch(
            &a,
            &p.receipt.id,
            &p.receipt.packet_hash,
            "transport",
            &Snapshot::default(),
        )
        .unwrap();
    }
    assert_eq!(
        s.context_receipts(&a, Some("run"), &Snapshot::default())
            .unwrap()[0]
            .stage,
        "dispatched"
    );
}
#[test]
fn conflicting_acknowledgement_is_rejected() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    let p = prepare(&s, &a, &se);
    s.acknowledge_append(
        &a,
        &p.receipt.id,
        &p.receipt.packet_hash,
        1,
        &Snapshot::default(),
    )
    .unwrap();
    assert!(matches!(
        s.acknowledge_append(
            &a,
            &p.receipt.id,
            &p.receipt.packet_hash,
            2,
            &Snapshot::default()
        ),
        Err(Error::IdempotencyConflict)
    ));
}
#[test]
fn preparation_reuses_same_request() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    assert_eq!(
        prepare(&s, &a, &se).receipt.id,
        prepare(&s, &a, &se).receipt.id
    );
}
#[test]
fn changed_query_cannot_reuse_request() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    prepare(&s, &a, &se);
    assert!(matches!(
        s.prepare_context(
            &a,
            &se,
            "run",
            "request",
            &Recall {
                query: "changed".into(),
                ..Default::default()
            },
            &ByteCounter,
            &ContextBudget::default()
        ),
        Err(Error::IdempotencyConflict)
    ));
}
#[test]
fn workspace_scope_cannot_pretend_to_be_a_session() {
    let (s, a, n, _) = setup();
    assert!(
        s.prepare_context(
            &a,
            &n,
            "run",
            "q",
            &Recall::default(),
            &ByteCounter,
            &ContextBudget::default()
        )
        .is_err()
    );
}
#[test]
fn suppressed_memory_is_not_packed() {
    let (mut s, a, n, se) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    s.set_preferences(&a, &m.id, 0, false, true).unwrap();
    assert!(prepare(&s, &a, &se).packet.selected.is_empty());
}
#[test]
fn pin_includes_an_off_query_record() {
    let (mut s, a, n, se) = setup();
    let m = active(&mut s, &a, &n, "a", "xylophone convention");
    s.set_preferences(&a, &m.id, 0, true, false).unwrap();
    let p = s
        .prepare_context(
            &a,
            &se,
            "run",
            "q",
            &Recall {
                query: "unrelated".into(),
                ..Default::default()
            },
            &ByteCounter,
            &ContextBudget::default(),
        )
        .unwrap();
    assert!(p.packet.selected.iter().any(|r| r.id == m.id));
}
#[test]
fn pin_does_not_promote_a_candidate() {
    let (mut s, a, n, se) = setup();
    let m = s.capture(&a, "a", draft(&n, "candidate")).unwrap().memory;
    s.set_preferences(&a, &m.id, 0, true, false).unwrap();
    assert!(prepare(&s, &a, &se).packet.selected.is_empty());
}
#[test]
fn preference_retries_converge() {
    let (mut s, a, n, _) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    assert_eq!(
        s.set_preferences(&a, &m.id, 0, true, false)
            .unwrap()
            .revision,
        1
    );
    assert_eq!(
        s.set_preferences(&a, &m.id, 0, true, false)
            .unwrap()
            .revision,
        1
    );
}
#[test]
fn preference_conflict_does_not_change_flags() {
    let (mut s, a, n, _) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    s.set_preferences(&a, &m.id, 0, true, false).unwrap();
    assert!(s.set_preferences(&a, &m.id, 0, false, true).is_err());
    assert!(s.preferences(&a, &m.id).unwrap().pinned);
}
#[test]
fn preference_counter_cannot_overflow() {
    let (mut s, a, n, _) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    assert!(s.set_preferences(&a, &m.id, i64::MAX, false, true).is_err());
}
#[test]
fn suppression_blocks_preflight_but_does_not_rewrite_receipts() {
    let (mut s, a, n, se) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    let p = prepare(&s, &a, &se);
    s.set_preferences(&a, &m.id, 0, false, true).unwrap();
    assert!(
        s.preflight_context(
            &a,
            &p.receipt.id,
            &p.receipt.packet_hash,
            &Snapshot::default()
        )
        .is_err()
    );
    s.acknowledge_append(
        &a,
        &p.receipt.id,
        &p.receipt.packet_hash,
        1,
        &Snapshot::default(),
    )
    .unwrap();
    assert_eq!(
        s.context_receipts(&a, Some("run"), &Snapshot::default())
            .unwrap()[0]
            .stage,
        "appended"
    );
}
#[test]
fn changed_knowledge_marks_old_context_invalid() {
    let (mut s, a, n, se) = setup();
    let old = active(&mut s, &a, &n, "a", "old convention");
    let p = prepare(&s, &a, &se);
    s.acknowledge_append(
        &a,
        &p.receipt.id,
        &p.receipt.packet_hash,
        1,
        &Snapshot::default(),
    )
    .unwrap();
    let new = s
        .capture(&a, "b", draft(&n, "new convention"))
        .unwrap()
        .memory;
    s.supersede(
        &a,
        (&old.id, old.revision),
        (&new.id, new.revision),
        None,
        &Snapshot::default(),
    )
    .unwrap();
    let c = s
        .context_receipts(&a, Some("run"), &Snapshot::default())
        .unwrap();
    assert_eq!(c[0].stage, "appended");
    assert!(c[0].invalidated_ids.contains(&old.id));
}
#[test]
fn forgetting_removes_dependent_context() {
    let (mut s, a, n, se) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    prepare(&s, &a, &se);
    s.forget(&a, &m.id, m.revision).unwrap();
    assert!(
        s.context_receipts(&a, Some("run"), &Snapshot::default())
            .unwrap()
            .is_empty()
    );
}
#[test]
fn metadata_feed_never_contains_memory_text() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "the unrepeated private phrase");
    prepare(&s, &a, &se);
    let p = s.event_page(&a, 0, 200).unwrap();
    let encoded = serde_json::to_string(&p).unwrap();
    assert!(!encoded.contains("unrepeated private phrase"));
    assert!(encoded.contains("memory.context_prepared"));
}
#[test]
fn events_use_existing_whalesong_category() {
    let (mut s, a, n, _) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    let p = s.event_page(&a, 0, 200).unwrap();
    assert!(!p.events.is_empty());
    assert!(
        p.events
            .iter()
            .all(|e| e["category"] == "memory" && e["schemaVersion"] == 1)
    );
}
#[test]
fn event_feed_is_scope_filtered() {
    let (mut s, a, n, _) = setup();
    active(&mut s, &a, &n, "a", "safe memory");
    let other = Access::readonly(vec![n.workspace("other")]).unwrap();
    assert!(s.event_page(&other, 0, 200).unwrap().events.is_empty());
}
#[test]
fn numeric_alias_is_stable() {
    let (mut s, a, n, _) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    let id = s.numeric_id(&a, &m.id).unwrap();
    assert_eq!(s.get_numeric(&a, id).unwrap().id, m.id);
}
#[test]
fn forgotten_alias_is_not_reused() {
    let (mut s, a, n, _) = setup();
    let m = active(&mut s, &a, &n, "a", "old text");
    let old = s.numeric_id(&a, &m.id).unwrap();
    s.forget(&a, &m.id, m.revision).unwrap();
    let m = active(&mut s, &a, &n, "b", "new text");
    assert!(s.numeric_id(&a, &m.id).unwrap() > old);
    assert!(matches!(s.get_numeric(&a, old), Err(Error::NotFound)));
}
#[test]
fn numeric_alias_does_not_grant_scope() {
    let (mut s, a, n, _) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    let alias = s.numeric_id(&a, &m.id).unwrap();
    let other = Access::readonly(vec![n.workspace("other")]).unwrap();
    assert!(matches!(s.get_numeric(&other, alias), Err(Error::NotFound)));
}
#[test]
fn knowledge_time_does_not_leak_later_approval() {
    let clock = Arc::new(AtomicI64::new(1000));
    let c = clock.clone();
    let mut s = Store::in_memory_with_clock(move || c.load(Ordering::SeqCst)).unwrap();
    let n = Scope::user("local", "owner");
    let a = Access::operator(vec![n.clone()]).unwrap();
    let m = s.capture(&a, "q", draft(&n, "fact")).unwrap().memory;
    clock.store(1100, Ordering::SeqCst);
    s.approve(&a, &m.id, 1, None, &Snapshot::default()).unwrap();
    assert_eq!(
        s.memory_as_of(&a, &m.id, 1050, 1050).unwrap().memory.status,
        Status::Candidate
    );
    assert_eq!(
        s.memory_as_of(&a, &m.id, 1100, 1100).unwrap().memory.status,
        Status::Active
    );
}
#[test]
fn history_before_capture_is_not_fabricated() {
    let (mut s, a, n, _) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    assert!(matches!(
        s.memory_as_of(&a, &m.id, 999, 999),
        Err(Error::NotFound)
    ));
}
#[test]
fn forgotten_history_cannot_be_read() {
    let (mut s, a, n, _) = setup();
    let m = active(&mut s, &a, &n, "a", "safe memory");
    s.forget(&a, &m.id, m.revision).unwrap();
    assert!(matches!(
        s.memory_as_of(&a, &m.id, 1000, 1000),
        Err(Error::NotFound)
    ));
}
#[test]
fn exact_context_budget_accounts_for_utf8() {
    let (mut s, a, n, se) = setup();
    active(&mut s, &a, &n, "a", "鲸鱼 memory");
    let p = prepare(&s, &a, &se);
    assert_eq!(p.packet.used_units, p.packet.text.len());
    assert_eq!(p.packet.unit, "utf8_bytes");
}
