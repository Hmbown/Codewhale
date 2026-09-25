# First-party plugin marketplace

Codewhale includes an offline snapshot of the `codewhale` catalog in the same
marketplace store consumed by the terminal, Extensions, recommendations, and
Runtime API. It lists Computer Use, WhaleWiki, Whalesong, Cloudflare Docs, the
Codewhale skill bundle, and Codewhale for Chrome (Chromewhale). Browsing does
not fetch or execute anything.

```text
/plugin marketplace list
/plugin marketplace show codewhale
/plugin marketplace install codewhale whalewiki
/plugin show whalewiki
/plugin trust whalewiki
/plugin enable whalewiki
/plugin update whalewiki
```

Review the manifest and capabilities before trust and enablement. Installation
uses the existing size-limited, traversal-safe installer and starts disabled
and untrusted. An update with changed bytes requires review again. Official
catalog provenance grants no execution or network permission. Removing the
catalog persists the choice and leaves installed plugins untouched; a locally
added catalog named `codewhale` takes precedence over the bundled snapshot.

The bundle source uses a gzip tarball URL with `#path=plugins/whalewiki` (or
`#path=skills`). The fragment selects exactly one bundle inside the shared
repository archive. Only that subtree is installed. Empty paths, traversal,
ambiguous roots, links, oversized archives, and changed plugin identities are
rejected. The install receipt preserves the source, including its selector,
so `/plugin update` retains the same bundle selector and reviewed revision.

## Built-in Computer Use across upgrades

Computer Use also ships inside the binary as a built-in bundle. Each build
writes its own copy under `$CODEWHALE_HOME/builtin-plugins`, so an upgrade
presents it as a new bundle. Your review carries over when the capability
hash is unchanged: a bundle you trusted and enabled stays trusted and enabled
on the new build. When the capabilities changed, it shows
`capabilities-changed` and stays off until you review it again with
`/plugin show computer-use` and `/plugin trust computer-use`. If you revoked
trust after your most recent review, nothing carries and the new build waits
for a fresh review; once you review a build again, later upgrades carry that
review. User and workspace plugins never carry trust: changed bytes
always need review.

## Chromewhale

Chromewhale (listed as "Codewhale for Chrome") is in the catalog bundled with
Core from marketplace revision `ae3dd22` on, where its checks pass on macOS,
Linux and Windows. It is a developer preview: installing the plugin does not
install the browser side, so you load its bundled Chrome extension unpacked
yourself. It then reads and acts on the tab you are looking at, one granted
site at a time. Like every catalog entry, it installs disabled and untrusted
until you review it.

## Keeping the repositories current

| Content | Authoritative source | Copies to check |
| --- | --- | --- |
| Catalog, WhaleWiki, Whalesong, Cloudflare Docs | `Hmbown/codewhale-plugin-marketplace` | Core catalog snapshot |
| Bundled skills | Core active catalog and `crates/tui/assets/skills` | Marketplace `skills` and `skills/upstream.json` |
| Computer Use | `Hmbown/codewhale-cu-plugin` | Marketplace plugin and Core bundled runtime |

The `Marketplace connection` workflow validates the exact catalog revision on
catalog changes. Its weekly and manual runs compare the current public
marketplace, bundled snapshot, skills, and Computer Use runtime. Drift fails
the check with a maintenance instruction; it does not rewrite user installs
or grant new permissions. It uses read-only repository access.

For each intentional update, review the upstream diff, synchronize the
source-owned copies, and run the marketplace checks:

```sh
# From codewhale-plugin-marketplace, with sibling source checkouts:
# After committing canonical skill changes, when intentionally updating skills:
npm run sync:skills
npm run check -- --core ../codewhale
npm run check:cu-sync
npm test && npm run check:web
```

Commit the reviewed marketplace changes, then update Core from that committed
revision (the generator never copies an uncommitted marketplace document):

```sh
# From codewhale:
python3 scripts/sync-marketplace.py --marketplace ../codewhale-plugin-marketplace
python3 scripts/sync-marketplace.py --marketplace ../codewhale-plugin-marketplace --check
npm test && npm run check:web
```

Review the generated snapshot and rebuild Core. Its provenance records the
exact marketplace commit, and each generated bundle URL pins that immutable
revision. A later marketplace change requires refreshing the Core snapshot and
rebuilding; an existing pinned install does not silently follow `main`. Push the
reviewed marketplace revision before publishing a Core release that references it. Hosted CI must be green for the
actual published revisions; local checks do not prove a public URL works.

Core's Computer Use copy (`crates/tui/plugins/computer-use`) is a runtime and
tests subset of the upstream repository. Copy the upstream files Core already
carries, plus any new runtime module the server imports and its tests, from the
reviewed upstream commit. Keep the three deliberate Core variants
(`package.json`, `README.md`, `tests/manifest.test.mjs`) and bump their version
to match. Record the commit in `crates/tui/plugins/computer-use.upstream-sha`.
Add each new runtime file to `COMPUTER_USE_FILES` in
`crates/tui/src/plugins/builtin.rs`. The
`computer_use_embed_list_matches_the_vendored_runtime_tree` test fails when
the two disagree. Then run `npm test` in the vendored directory.

Skill wording changes also need behavioral evaluation before claiming better
outcomes. See [Skill evaluation](SKILL_EVALUATION.md).
