# winget packaging for CodeWhale

This directory holds a source winget manifest for `HunterBown.CodeWhale` (resolves #1561).

> **Not what winget serves today.** The package published in
> [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs/tree/master/manifests/h/HunterBown/CodeWhale)
> (checked 2026-10-04, latest 0.10.0) is a different, multi-file manifest:
> `InstallerType: portable`, one x64 installer
> (`codewhale-tui-windows-x64.exe`), `Commands: codewhale`, and a VC++ 2015+
> x64 runtime dependency. It has no ARM64 installer and no `codew` alias.
> The singleton below (NSIS + ZIP, x64/arm64, `codewhale` + `codew`) is stale
> at 0.9.6 and has never been what winget installs. Before the next
> submission, base it on the published manifest or update both together, and
> keep the identifier `HunterBown.CodeWhale` (this file used `Hmbown.CodeWhale`
> until 2026-10-04, which would have created a second package).

The singleton was written so winget installs the single runtime under
`codewhale` + `codew` and never a `codewhale-tui` command. GitHub Releases
retain byte-identical `codewhale-tui-*` filenames only for legacy updater
compatibility.

## Files

- `HunterBown.CodeWhale.yaml` — singleton manifest for `winget install HunterBown.CodeWhale`. The
  installers all point at the signed (or checksum-verified) GitHub Release assets for the same
  version (`CodeWhaleSetup.exe` for x64 NSIS, plus portable ZIP fallbacks for x64/arm64).
- `generate-winget-manifest.sh` — bumps `PackageVersion`, `ReleaseDate`, and the four
  `InstallerSha256` placeholders from a local `release-assets/` checkout.
- `.winget/HunterBown.CodeWhale.yaml` (repo root) is a verbatim mirror for tooling that expects `.winget/`.
  Keep both in sync; `packaging/winget/HunterBown.CodeWhale.yaml` is canonical.

## Version flow

1. Tag `vX.Y.Z` publishes `CodeWhaleSetup.exe`, `codewhale-windows-x64.zip`,
   `codewhale-windows-x64-portable.zip`, `codewhale-windows-arm64.zip`,
   `codewhale-windows-arm64-portable.zip`, and `codewhale-artifacts-sha256.txt`.
2. From the release tag checkout, run:
   ```bash
   ./packaging/winget/generate-winget-manifest.sh X.Y.Z /path/to/release-assets
   ```
   It rewrites both `packaging/winget/HunterBown.CodeWhale.yaml` and `.winget/HunterBown.CodeWhale.yaml`
   with the fresh version and the four SHA-256 values extracted from `codewhale-artifacts-sha256.txt`.
3. Validate locally with `winget validate` (requires winget + the manifest schema):
   ```bash
   winget validate --manifest packaging/winget/HunterBown.CodeWhale.yaml
   # or the Microsoft validator in winget-pkgs CI:
   # https://github.com/microsoft/winget-pkgs#validation
   ```
4. Submit to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) via
   `wingetcreate` or a manual PR that adds `manifests/h/HunterBown/CodeWhale/X.Y.Z/`:
   ```bash
   wingetcreate update HunterBown.CodeWhale --version X.Y.Z --urls \
     https://github.com/codewhale-hq/CodeWhale/releases/download/vX.Y.Z/CodeWhaleSetup.exe \
     https://github.com/codewhale-hq/CodeWhale/releases/download/vX.Y.Z/codewhale-windows-x64.zip \
     https://github.com/codewhale-hq/CodeWhale/releases/download/vX.Y.Z/codewhale-windows-x64-portable.zip \
     https://github.com/codewhale-hq/CodeWhale/releases/download/vX.Y.Z/codewhale-windows-arm64.zip \
     https://github.com/codewhale-hq/CodeWhale/releases/download/vX.Y.Z/codewhale-windows-arm64-portable.zip
   ```
   The generated PR must pass the winget-pkgs validation workflow before merge.

## Single-binary note

Until v0.9.4 the release matrix installed three commands (`codewhale`, `codew`,
and `codewhale-tui`). Since v0.9.5 each target installs only the byte-identical
`codewhale` + `codew` commands (Windows also ships `codewhale.bat`). GitHub
Releases retain `codewhale-tui-*` compatibility filenames for old updater
clients, but the winget ZIP `NestedInstallerFiles` lists only the two current
PATH commands; `codewhale-tui.exe` is intentionally absent.

## FreeBSD

FreeBSD has no prebuilt GitHub Release asset (see `docs/INSTALL.md` § FreeBSD). Install via Cargo:

```bash
pkg install -y rust pkgconf  # or ports-mgmt/pkg
cargo install codewhale-cli --locked   # provides `codewhale`
```

The npm wrapper on FreeBSD exits with `Unsupported platform: freebsd` and points to the Cargo path.
A native `pkg install codewhale` port is tracked as a follow-up to #1097 — contributions welcome
under `packaging/freebsd/`.
