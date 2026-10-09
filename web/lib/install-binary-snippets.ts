import type { Arch } from "./install-platform";

function windowsSnippet(arch: "x64" | "arm64"): string {
  return `# PowerShell
$ErrorActionPreference = "Stop"
$dest = "$Env:USERPROFILE\\bin"
New-Item -ItemType Directory -Force $dest | Out-Null
$manifest = Invoke-WebRequest https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-artifacts-sha256.txt

Invoke-WebRequest \`
  -Uri https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-windows-${arch}.exe \`
  -OutFile "$dest\\codewhale.exe"
Invoke-WebRequest \`
  -Uri https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codew-windows-${arch}.exe \`
  -OutFile "$dest\\codew.exe"

$expected = @{}
$manifest.Content -split "\`n" | ForEach-Object {
  $parts = $_.Trim() -split "\\s+"
  if ($parts.Length -ge 2) { $expected[$parts[1]] = $parts[0].ToUpperInvariant() }
}
if ((Get-FileHash "$dest\\codewhale.exe" -Algorithm SHA256).Hash -ne $expected["codewhale-windows-${arch}.exe"]) { throw "codewhale.exe checksum mismatch" }
if ((Get-FileHash "$dest\\codew.exe" -Algorithm SHA256).Hash -ne $expected["codew-windows-${arch}.exe"]) { throw "codew.exe checksum mismatch" }

$Env:Path = "$dest;$Env:Path"`;
}

function windowsVerify(arch: "x64" | "arm64"): string {
  return `# PowerShell
$manifest = Invoke-WebRequest https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-artifacts-sha256.txt
$expected = @{}
$manifest.Content -split "\`n" | ForEach-Object {
  $parts = $_.Trim() -split "\\s+"
  if ($parts.Length -ge 2) { $expected[$parts[1]] = $parts[0].ToUpperInvariant() }
}
if ((Get-FileHash "$Env:USERPROFILE\\bin\\codewhale.exe" -Algorithm SHA256).Hash -ne $expected["codewhale-windows-${arch}.exe"]) { throw "codewhale.exe checksum mismatch" }
if ((Get-FileHash "$Env:USERPROFILE\\bin\\codew.exe" -Algorithm SHA256).Hash -ne $expected["codew-windows-${arch}.exe"]) { throw "codew.exe checksum mismatch" }`;
}

export const SNIPPETS: Record<Arch, string> = {
  "macos-arm64": `curl -fsSL -O https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-artifacts-sha256.txt
curl -fsSL -O \\
  https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-macos-arm64
curl -fsSL -O \\
  https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codew-macos-arm64
grep -E ' (codewhale|codew)-macos-arm64$' codewhale-artifacts-sha256.txt | shasum -a 256 -c -
chmod +x codewhale-macos-arm64 codew-macos-arm64
xattr -d com.apple.quarantine codewhale-macos-arm64 codew-macos-arm64 2>/dev/null || true
sudo mv codewhale-macos-arm64 /usr/local/bin/codewhale
sudo mv codew-macos-arm64 /usr/local/bin/codew`,
  "macos-x64": `curl -fsSL -O https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-artifacts-sha256.txt
curl -fsSL -O \\
  https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-macos-x64
curl -fsSL -O \\
  https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codew-macos-x64
grep -E ' (codewhale|codew)-macos-x64$' codewhale-artifacts-sha256.txt | shasum -a 256 -c -
chmod +x codewhale-macos-x64 codew-macos-x64
xattr -d com.apple.quarantine codewhale-macos-x64 codew-macos-x64 2>/dev/null || true
sudo mv codewhale-macos-x64 /usr/local/bin/codewhale
sudo mv codew-macos-x64 /usr/local/bin/codew`,
  "linux-x64": `curl -fsSL -O https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-artifacts-sha256.txt
curl -fsSL -O \\
  https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-linux-x64
curl -fsSL -O \\
  https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codew-linux-x64
grep -E ' (codewhale|codew)-linux-x64$' codewhale-artifacts-sha256.txt | sha256sum -c -
chmod +x codewhale-linux-x64 codew-linux-x64
sudo mv codewhale-linux-x64 /usr/local/bin/codewhale
sudo mv codew-linux-x64 /usr/local/bin/codew`,
  "linux-arm64": `curl -fsSL -O https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-artifacts-sha256.txt
curl -fsSL -O \\
  https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-linux-arm64
curl -fsSL -O \\
  https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codew-linux-arm64
grep -E ' (codewhale|codew)-linux-arm64$' codewhale-artifacts-sha256.txt | sha256sum -c -
chmod +x codewhale-linux-arm64 codew-linux-arm64
sudo mv codewhale-linux-arm64 /usr/local/bin/codewhale
sudo mv codew-linux-arm64 /usr/local/bin/codew`,
  "windows-x64": windowsSnippet("x64"),
  "windows-arm64": windowsSnippet("arm64"),
};

function unixVerify(platform: string, checksumCommand: string): string {
  return `curl -fsSL -O https://github.com/codewhale-hq/CodeWhale/releases/latest/download/codewhale-artifacts-sha256.txt
verify_binary() {
  asset="$1"
  installed="$2"
  expected=$(awk -v asset="$asset" '$2 == asset { print $1 }' codewhale-artifacts-sha256.txt)
  actual=$(${checksumCommand} "$installed" | awk '{ print $1 }')
  if [ -z "$expected" ] || [ "$actual" != "$expected" ]; then
    echo "$installed checksum mismatch" >&2
    return 1
  fi
}
verify_binary codewhale-${platform} /usr/local/bin/codewhale
verify_binary codew-${platform} /usr/local/bin/codew`;
}

export const VERIFY: Record<Arch, string> = {
  "macos-arm64": unixVerify("macos-arm64", "shasum -a 256"),
  "macos-x64": unixVerify("macos-x64", "shasum -a 256"),
  "linux-x64": unixVerify("linux-x64", "sha256sum"),
  "linux-arm64": unixVerify("linux-arm64", "sha256sum"),
  "windows-x64": windowsVerify("x64"),
  "windows-arm64": windowsVerify("arm64"),
};
