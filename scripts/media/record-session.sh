#!/usr/bin/env bash
# Record the v0.9.2 real-session media (#4906) into a sealed environment.
#
# This wrapper exists for one reason: a recording is published footage, and the
# failure mode is not "the take is bad" — it is "the take contains your home
# directory, your shell history, or a credential". Every guard below refuses to
# record rather than producing something that has to be scrubbed afterwards.
#
# It records a REAL session. It does not stage or reconstruct output. If the
# take is unimpressive, change the task and re-run; do not edit frames.
#
# Procedure and acceptance checklist: docs/releases/v0.9.2-media-plan.md
# Verify the result with:                scripts/media/check-media-assets.py
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tape="${repo_root}/docs/evidence/v092-first-fleet-session.tape"
demo_dir="${CW_MEDIA_DEMO_DIR:-/tmp/cw-media-demo}"
out_dir="${CW_MEDIA_OUT_DIR:-${repo_root}/.media-out}"

die() { echo "ERROR: $*" >&2; exit 1; }

# --- Preconditions ----------------------------------------------------------

command -v vhs >/dev/null || die "vhs is not installed (brew install vhs)"
command -v ffmpeg >/dev/null || die "ffmpeg is not installed (brew install ffmpeg)"
[[ -f "${tape}" ]] || die "missing tape: ${tape}"

# Record the exact binary that was dogfooded, not whatever is first on PATH.
binary="$(command -v codewhale || true)"
[[ -n "${binary}" ]] || die "codewhale is not on PATH — run scripts/release/install-dogfood.sh first"

version_line="$("${binary}" --version)"
head_sha="$(git -C "${repo_root}" rev-parse HEAD)"
short_sha="${head_sha:0:12}"
if [[ "${version_line}" != *"${short_sha}"* ]]; then
  die "installed codewhale is not this checkout.
  installed: ${version_line}
  HEAD:      ${short_sha}
  Recording a SHA that is not the release candidate makes the asset a lie about
  what the product looks like. Run scripts/release/install-dogfood.sh."
fi

if [[ -n "$(git -C "${repo_root}" status --porcelain --untracked-files=no)" ]]; then
  die "refusing to record from a dirty tree — the capture receipt could not name an exact source"
fi

# --- Credential guard -------------------------------------------------------
# The session must be keyless on a sealed local route. Anything that looks like
# a provider credential in the environment could reach the screen.
leaked=()
while IFS='=' read -r name _; do
  case "${name}" in
    *API_KEY|*_TOKEN|*_SECRET|ANTHROPIC_*|OPENAI_*|DEEPSEEK_*|MOONSHOT_*|KIMI_*|GROQ_*|XAI_*|GEMINI_*)
      leaked+=("${name}") ;;
  esac
done < <(env)
if (( ${#leaked[@]} )); then
  die "provider credentials are present in the environment: ${leaked[*]}
  Re-run from a clean shell (\`env -i\` or a fresh terminal). The recording must
  be demonstrably keyless."
fi

# --- Sealed environment -----------------------------------------------------
sealed="$(mktemp -d)"
mkdir -p "${sealed}/.codewhale" "${demo_dir}" "${out_dir}"
cleanup() { rm -rf "${sealed}"; }
trap cleanup EXIT

# Blue Stage dark, selected in the product rather than imitated by the
# emulator palette (see the tape's note).
cat > "${sealed}/.codewhale/config.toml" <<'TOML'
theme = "Blue Stage"
TOML

echo "Recording ${version_line}"
echo "  tape:      ${tape}"
echo "  workspace: ${demo_dir}"
echo "  sealed:    HOME=${sealed}"
echo

pushd "${demo_dir}" >/dev/null
HOME="${sealed}" \
CODEWHALE_HOME="${sealed}/.codewhale" \
CODEWHALE_CONFIG_PATH="${sealed}/.codewhale/config.toml" \
CODEWHALE_MCP_CONFIG="${sealed}/.codewhale/mcp.json" \
  vhs "${tape}"
popd >/dev/null

gif="${demo_dir}/first-fleet-session.gif"
[[ -f "${gif}" ]] || die "vhs produced no GIF at ${gif}"

# --- Derivatives (media plan step 4) ----------------------------------------
mv "${gif}" "${out_dir}/first-fleet-session.gif"
gif="${out_dir}/first-fleet-session.gif"

ffmpeg -loglevel error -y -i "${gif}" -movflags +faststart -pix_fmt yuv420p \
  -vf "scale=1280:720" -an "${out_dir}/first-fleet-session.mp4"
ffmpeg -loglevel error -y -i "${gif}" -frames:v 1 "${out_dir}/first-fleet-session.png"

# --- Capture receipt --------------------------------------------------------
# So the asset can be re-shot when the UI changes, instead of quietly aging
# into a lie about what the product looks like.
cat > "${out_dir}/capture.json" <<JSON
{
  "id": "first-fleet-session",
  "issue": "https://github.com/codewhale-hq/CodeWhale/issues/4906",
  "recorded_from_commit": "${head_sha}",
  "version_line": "${version_line}",
  "tape": "docs/evidence/v092-first-fleet-session.tape",
  "grid": "120x32",
  "product_theme": "Blue Stage",
  "route": "sealed loopback Ollama; no hosted provider, no account, no credential",
  "workspace": "${demo_dir}",
  "sha256": {
    "gif": "$(shasum -a 256 "${out_dir}/first-fleet-session.gif" | cut -d' ' -f1)",
    "mp4": "$(shasum -a 256 "${out_dir}/first-fleet-session.mp4" | cut -d' ' -f1)",
    "png": "$(shasum -a 256 "${out_dir}/first-fleet-session.png" | cut -d' ' -f1)"
  }
}
JSON

echo
echo "Wrote:"
ls -la "${out_dir}"
echo
echo "Next:"
echo "  1. WATCH THE TAKE. If it is not worth showing, change the task and re-run."
echo "  2. python3 scripts/media/check-media-assets.py --dir ${out_dir}"
echo "  3. Follow the acceptance checklist in docs/releases/v0.9.2-media-plan.md"
echo "     before copying anything into web/public/media/ or flipping the"
echo "     manifest entry to published."
