# Compiled-host runtime inputs

This is a release preparation contract, not runtime authority or a publication
approval. The runtime remains optional and Node remains the Core default.

The exact local Bun observed during this packet is **1.4.0+34cbb9a40**, executable
SHA-256 `539598c775882420b9d8deb7dc14d845f20f7d26f5600c50ab067dde6ac3f3bf`. Its
full source revision is `34cbb9a40b4bd1bd767d134a7065e66c2432a676`, with WebKit
revision `0f966e81b78c84bb23213e391bc679c4ef83e56b`. This is one observed macOS
ARM64 binary, not qualification for other architectures or libc families.

The [matching Bun license](https://raw.githubusercontent.com/oven-sh/bun/34cbb9a40b4bd1bd767d134a7065e66c2432a676/LICENSE.md)
identifies statically linked JavaScriptCore/WebCore under LGPL2 and TinyCC under
LGPL2.1. It provides the Bun/WebKit source rebuild path. Delivery must retain
the corresponding copyright/license texts and provide the exact matching
source, patches, build/relink recipe and application object/source inputs
needed to modify and relink the LGPL components. URLs or a single MIT label do
not constitute that payload.

`stage-compiled-host.mjs` requires a reviewed JSON closure file with schema1,
exact `runtime_version`, full `runtime_revision`, `runtime_sha256`, full
`webkit_revision`, actual `libc` (`glibc`, `musl`, or `none`), current
`bundle_sha256` and `source_commit`; `notices`, `notices_sha256`, `relink_source`,
`source_sha256`; `license_closure: "complete"` and
`corresponding_source: "complete"`. Every named component must have an actual
`notice_components[name]: {file, sha256}` text included byte-for-byte in the
combined notices, including linked libraries, embedded polyfills and Rust
dependencies. Paths resolve only from this explicitly supplied local closure
file. The required notice inventory is enforced in the stager; the complete
per-platform component inventory must also be reviewed.

The source `.tar.gz` must include `source_inputs` entries for `bun`, `webkit`,
`tinycc`, and `relink-recipe`, each naming its contained `root`; Bun and WebKit
entries also carry the exact corresponding revision. This inventory and hash
check detects missing/mixed inputs. Review of the complete reproduction and
relink recipe remains necessary: flags and archive member names alone do not
prove source sufficiency. Do not mark incomplete inputs complete.

Exact corresponding Bun and TinyCC source archives are available from the
[matching Bun commit](https://codeload.github.com/oven-sh/bun/tar.gz/34cbb9a40b4bd1bd767d134a7065e66c2432a676)
and [matching TinyCC fork commit](https://codeload.github.com/oven-sh/tinycc/tar.gz/05f0fafaa3be31e31d7b4b5c17dc60f62c991171).
The Bun archive includes its exact TinyCC patch and build scripts. Matching
libwebp notice texts are available from Google's primary
[exact commit](https://chromium.googlesource.com/webm/libwebp/+/b7e29b9d75bd31422b00c2a446d49d7af06c328d/COPYING).
Earlier GitHub mirror 404 observations do not establish a source/notice absence.

The matching WebKit release is
[`autobuild-0f966…`](https://github.com/oven-sh/WebKit/releases/tag/autobuild-0f966e81b78c84bb23213e391bc679c4ef83e56b).
Its 42 observed assets are ABI-specific prebuilts according to the pinned build
contract; no full source/relink archive was found among them. The generated
exact-source archive request returned HTTP 422 during the takeover. A full exact
fork source snapshot, generated-header/build inputs, complete copyright/license
texts for embedded components/polyfills/Rust dependencies, and a reviewed
rebuild/relink recipe are still required. API tree results were truncated and
must not be labeled a complete source inventory. No delivery-qualified closure
has been produced by these downloads.

The exact build source also includes static SQLite on Linux/Windows, while macOS
loads system SQLite. Its in-tree source/public-domain dedication must be
accounted for in a complete per-platform component inventory. The stager's
required notice list is a minimum guard; full inventory and source sufficiency
remain reviewed input obligations, never conclusions from flags alone.

The release workflow is explicitly opt-in and fails when those local reviewed
inputs or exact-source Native proof are absent. The compiled image is copied
from the already tested CI artifact, never rebuilt after Native containment
proof. Five direct-image protocol cases apply on every supported OS; macOS
adds a sixth reexec/jetsam case. These cases require zero failures and zero
skips and cannot substitute for the same exact compiled image's actual Rust
Native containment and memory enforcement or license/source closure.

The official Bun 1.4.0 release has Android and musl runtime assets. Android is
**not a qualified Codewhale compiled-host delivery target** in this contract;
that is an evidence boundary, not a claim that upstream has no Android binary.
The existing static musl CLI remains available, while a musl runtime needs its
own exact-image Native receipt and complete source/notice closure. GNU runner
proof does not transfer. No additional delivery targets are enabled here.

Delivery probes the actual selected Bun process and the actual compiled image
for platform/architecture. Both must equal the native runner and the exact-image
Native receipt; an emulated x64 Bun on ARM cannot be labeled ARM. Windows now
requires all nine current Native cases, including compiled-image containment
and memory enforcement plus seven LPAC cases. Linux/macOS require their two
compiled-image Rust cases, independently of the direct-image protocol cases.
This does not claim any of those remote cases ran locally.
