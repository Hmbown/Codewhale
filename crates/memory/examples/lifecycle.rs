use codewhale_memory::*;
fn main() -> Result<()> {
    let scope = Scope::user("local", "developer").workspace("codewhale");
    let access = Access::operator(vec![scope.clone()])?;
    let mut memory = Store::in_memory()?;
    let evidence = Evidence {
        kind: SourceKind::User,
        uri: "codewhale://session/demo/message/1".into(),
        locator: "user decision".into(),
        sha256: None,
        observed_at: 0,
    };
    let mut draft = Draft::note(
        scope,
        "Recall placement",
        "Append recall to tool history; preserve the frozen system prefix.",
        evidence,
    );
    draft.kind = Kind::Decision;
    draft.key = Some("context.recall-placement".into());
    let proposed = memory.capture(&access, "demo:1", draft)?;
    let approved = memory.approve(
        &access,
        &proposed.memory.id,
        proposed.memory.revision,
        None,
        &Snapshot::default(),
    )?;
    let report = memory.recall(
        &access,
        &Recall {
            query: "recall prefix".into(),
            ..Recall::default()
        },
    )?;
    let packet = compile_context(&report.hits, &ByteCounter, &ContextBudget::default())?;
    println!("{}", packet.text);
    let deletion = memory.forget(&access, &approved.id, approved.revision)?;
    println!("{}", serde_json::to_string(&deletion)?);
    Ok(())
}
