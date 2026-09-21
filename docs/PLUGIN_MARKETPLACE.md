# First-party plugin marketplace

Codewhale includes an offline snapshot of the `codewhale` catalog in the same
marketplace store consumed by the terminal, Extensions, recommendations, and
Runtime API. It lists Computer Use, WhaleWiki, Whalesong, Cloudflare Docs, and the
Codewhale skill bundle. Browsing does not fetch or execute anything.

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

Skill wording changes also need behavioral evaluation before claiming better
outcomes. See [Skill evaluation](SKILL_EVALUATION.md).
