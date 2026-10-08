# Pinned DSH multi-file bundle fixture

Parser-only data from DeepSeek Harness `0.1.7-alpha.2`, commit
[`00102833dfaee1da9f48a3a8eae9d34005a75218`](https://github.com/deepseek-ai/deepseek-harness/tree/00102833dfaee1da9f48a3a8eae9d34005a75218/packages/bundle/web-app).

`package.json` and the five declared patch files are byte-for-byte copies of
`packages/bundle/web-app` at that commit. `LICENSE` is the upstream root MIT
license. `UPSTREAM.json` records origin and SHA-256 hashes; the offline converter
tests check every hash without Git, a network connection, or another checkout.

Do not install this package or execute its expressions. It is not a standalone
native plugin: most rows need DSH runtime services, presets, or a base profile.
The fixture proves ordered patch parsing and explicit unsupported-row reporting,
not executable compatibility. Small synthetic fixtures in
`../../test_convert_plugin.py` exercise portable conversion and a benign Node
MCP lifecycle separately.
