# Scoped counter mod

This plain ESM example uses the documented experimental host shims. It registers
one tool, one user command, one prompt section and one pre-execute listener.
The listener applies only to `mod_counter`; the label `blocked` demonstrates
denial. It does not grant approval or alter other tools.

Start Codewhale with `codewhale --enable extension_host`, then enter:

```text
/plugin install ./docs/examples/plugins/mod-extension
/plugin validate mod-extension
/plugin show mod-extension
/plugin enable mod-extension
```

Inspect the Native capability and source. Personally run the exact trust command
printed by the review, then enable the plugin again. Installation alone leaves
it disabled and untrusted; the experimental feature is off by default.

Ask the model to call `mod_counter` with `{"label":"first"}`. The ordinary tool
approval gate applies. A successful call returns JSON with its saved count and
the invocation identity fields Rust supplied. Ask for `{"increment":false}` to
read without changing the count, or `{"label":"blocked"}` to exercise the hook.
Run `/mod-count` yourself to display the count without a model call.

```text
/plugin disable mod-extension
```

Disabling removes the tool, command, listener and prompt section. The counter
remains in the owner directory Rust assigned and survives reloads; disposal does
not delete state. This example serializes its counter updates inside one owner.
Separate processes writing the same storage key are last-writer-wins, so this
is not a cross-process atomic counter.

There is no published author SDK yet. This example uses no ambient package,
custom UI widget, skill-root registration or DSH bundle loader. See
[the extension contract](../../../EXTENSIONS.md) and
[简体中文扩展指南](../../../zh_hans/EXTENSIONS.md).
