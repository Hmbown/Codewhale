// Generated from docs/INSTALL.md by scripts/derive-install.mjs. Do not edit.

export const INSTALL_GUIDE = {
  "sourceHash": "158b2b791f3194810c4288e2dbade45c4f637ae1868156b313cfc7308d0edb46",
  "anchors": [
    "installing-codewhale",
    "60-second-quickstart-linux-or-macos",
    "contents",
    "1-before-you-start",
    "recommended-official-github-releases",
    "2-recommended-installer-curl--sh",
    "macos-notes",
    "put-it-on-your-path",
    "options",
    "verify",
    "re-running-the-installer",
    "upgrade--uninstall",
    "3-manual-download-from-github-releases",
    "3a-bare-binaries",
    "3b-archive",
    "4-npm",
    "if-you-get-eacces-permission-denied",
    "notes",
    "4-install-via-cargo-any-tier-1-rust-target",
    "7-build-from-source",
    "5-cargo-and-building-from-source",
    "prerequisites-debianubuntu",
    "5a-from-cratesio",
    "5b-from-a-git-checkout",
    "6-homebrew-on-linux-and-nix",
    "homebrew-on-linux-works-but-not-with-the-command-the-old-docs-gave",
    "nix-partially-tested",
    "7-updating-and-rolling-back",
    "roll-back-to-a-previous-release",
    "rolling-back-eg-to-v0913",
    "8-api-keys-and-providers",
    "where-codewhale-looks-for-a-key-first-match-wins",
    "ways-to-set-a-deepseek-key-all-tested",
    "check-which-key-is-active",
    "remove-a-stored-key",
    "other-providers",
    "8-shell-completions",
    "9-shell-completions",
    "10-running-it",
    "the-tui",
    "headless-scripts-ci",
    "resuming",
    "11-terminal-notes",
    "ghostty-tested-ghostty-131-on-linuxx11",
    "other-terminals",
    "12-uninstalling",
    "step-1-forget-stored-keys-if-you-used-auth-set-or-f3",
    "step-2-remove-the-program",
    "step-3-remove-data-no-uninstaller-does-this-for-you",
    "13-troubleshooting",
    "appendix-other-platforms-not-re-tested-in-this-revision",
    "supported-platforms-and-assets",
    "linux-arm64-portability",
    "migrating-from-npm-cargo-or-another-installation",
    "migrating-from-npm-cargo-or-another-installation-1",
    "android--termux-arm64",
    "android--termux-arm64-preview",
    "china--mirror-friendly-install",
    "omarchy--aur",
    "windows",
    "windows-scoop",
    "windows-winget-v095",
    "windows-nsis-installer",
    "freebsd",
    "freebsd-cross-compiling-windows-source-builds",
    "freebsd-14-source-build-workaround-1097",
    "cross-compiling-from-x64-to-arm64-linux",
    "windows-build-from-source",
    "older-release-and-regional-troubleshooting",
    "unsupported-architecture-arm64-on-platform-linux",
    "missing_companion_binary-after-upgrading-an-older-install",
    "codewhale-update-reports-no-asset-found-for-platform-codewhale-linux-aarch64",
    "npm-download-is-slow-or-times-out-from-mainland-china",
    "codewhale-update-is-blocked-by-github-from-mainland-china",
    "windows-and-npm-download-troubleshooting",
    "windows-tls-handshake-eof-or-crypt_e_revocation_offline-from-rustup-init",
    "windows-msvc-compiler-clexe-not-found-during-cargo-build",
    "windows-拒绝访问-os-error-5-when-cargo-executes-build-scripts",
    "npm-binary-download-times-out"
  ],
  "chunks": [
    {
      "kind": "html",
      "text": "<h1 id=\"installing-codewhale\">Installing Codewhale</h1>\n<blockquote>\n<p>阅读简体中文版：<a href=\"https://github.com/Hmbown/CodeWhale/blob/main/docs/zh_hans/INSTALL.md\">zh_hans/INSTALL.md</a> (not yet updated for this revision)</p>\n</blockquote>\n<p>Codewhale is an open-source coding agent that runs in your terminal. You give\nit a task (&quot;fix the failing test&quot;, &quot;add a CLI flag&quot;). It reads your repository,\nedits files and runs commands. In the default <strong>Ask</strong> posture it applies file\nedits inside the workspace immediately (and shows you the diff), but asks before\nrunning shell commands, so commit or stash anything you care about first. It\nworks with many model providers. <strong>DeepSeek</strong> is the default.</p>\n<p>The command is <code>codewhale</code>. <code>codew</code> is a shorter alias for the same program.</p>\n<p>This guide was written by installing <strong>v0.10.0</strong> (released 2026-09-22) on a\nfresh <strong>Ubuntu 24.04 x86_64</strong> machine, on every path described here. Every\ncommand shown was run and its output checked (see the <a href=\"https://github.com/Hmbown/Codewhale/blob/37ecdfcc49bc68a9b0d058b97c3946e62c34bd31/docs/install-report/v0.10.0-2026-09-23/RECEIPTS.md\">install receipts</a>). Steps that\ncould not be run on that machine are marked <strong>(untested on this VM: reason)</strong>.\nmacOS, Windows and Android are out of scope, apart from a few notes. A second\npass re-ran the installer, manual-download, archive and npm paths, the no-key\nchecks and zsh completion on <strong>macOS 26.1 (Apple silicon)</strong>; see\n<a href=\"#macos-notes\">macOS notes</a>. Steps that need a model call were not re-run\nthere.</p>\n<p>Install commands that use <code>latest</code> resolve to the latest <strong>published</strong> GitHub\nRelease or package. Between releases, <code>main</code> may already describe the next\nversion (for example the v0.10.0 source candidate before 2026-09-22). A\ncandidate isn&#39;t installable until its tag, checksums and release assets\nexist.</p>\n<hr>\n<h2 id=\"60-second-quickstart-linux-or-macos\">60-second quickstart (Linux or macOS)</h2>\n"
    },
    {
      "kind": "code",
      "text": "# 1. Install. Downloads two checksum-verified binaries into ~/.local/bin (no sudo).\ncurl -fsSL https://codewhale.net/install.sh | sh\n\n# 2. Make sure ~/.local/bin is on your PATH, now and in future terminals.\necho 'export PATH=\"$HOME/.local/bin:$PATH\"' >> ~/.bashrc    # zsh: use ~/.zshrc\nexport PATH=\"$HOME/.local/bin:$PATH\"\ncodewhale --version          # -> codewhale 0.10.0 (1be1a703b975)\n\n# 3. Give it a DeepSeek API key (from https://platform.deepseek.com/api_keys).\ncodewhale auth set --provider deepseek    # prompts for the key; nothing is echoed\ncodewhale auth status --provider deepseek # \"active source: secret store\"\n\n# 4. Run your first task inside a git repository.\ncd ~/your-project\ncodewhale"
    },
    {
      "kind": "html",
      "text": "<p>In the TUI, type something concrete:</p>\n"
    },
    {
      "kind": "code",
      "text": "create a Python file primes.py that prints the first 10 primes, run it, and show me the output"
    },
    {
      "kind": "html",
      "text": "<p>Codewhale writes the file, shows you a diff, then asks <strong>APPROVAL: bash\npython3 primes.py – Do you want to proceed?</strong> Press <code>y</code> to allow it once. It\nruns the command and reports the output. Press <code>Ctrl-D</code> (with an empty input\nbox) to quit. It prints the command to resume the session later.</p>\n<blockquote>\n<p><strong>If nothing happens after you send your first message,</strong> you have no key\nconfigured. v0.10.0 doesn&#39;t warn you in that case. Press <strong>F3</strong>. If DeepSeek\nshows <code>missing key</code>, press Enter, paste the key, then pick a model and\nconfirm.</p>\n</blockquote>\n<hr>\n<h2 id=\"contents\">Contents</h2>\n<ol>\n<li><a href=\"#1-before-you-start\">Before you start</a></li>\n<li><a href=\"#2-recommended-installer-curl--sh\">Install: recommended installer</a></li>\n<li><a href=\"#3-manual-download-from-github-releases\">Install: manual download from GitHub Releases</a></li>\n<li><a href=\"#4-npm\">Install: npm</a></li>\n<li><a href=\"#5-cargo-and-building-from-source\">Install: Cargo / build from source</a></li>\n<li><a href=\"#6-homebrew-on-linux-and-nix\">Install: Homebrew (Linux) and Nix</a></li>\n<li><a href=\"#7-updating-and-rolling-back\">Updating and rolling back</a></li>\n<li><a href=\"#8-api-keys-and-providers\">API keys and providers</a></li>\n<li><a href=\"#9-shell-completions\">Shell completions</a></li>\n<li><a href=\"#10-running-it\">Running it: TUI, headless, resume</a></li>\n<li><a href=\"#11-terminal-notes\">Terminal notes (Ghostty and others)</a></li>\n<li><a href=\"#12-uninstalling\">Uninstalling and what Codewhale leaves behind</a></li>\n<li><a href=\"#13-troubleshooting\">Troubleshooting</a></li>\n<li><a href=\"#appendix-other-platforms-not-re-tested-in-this-revision\">Appendix: other platforms (not re-tested in this revision)</a></li>\n</ol>\n<hr>\n<h2 id=\"1-before-you-start\">1. Before you start</h2>\n<div class=\"install-guide-table\" role=\"region\" aria-label=\"Installation table 1: You need / Why\" tabindex=\"0\"><table>\n<thead>\n<tr>\n<th>You need</th>\n<th>Why</th>\n</tr>\n</thead>\n<tbody><tr>\n<td>Linux x86_64 or arm64, or macOS</td>\n<td>Prebuilt binaries exist for these. The Linux binaries are <strong>static</strong> (musl), so they have no glibc or libdbus dependency and run on any distro.</td>\n</tr>\n<tr>\n<td><code>curl</code> (or <code>wget</code>) and <code>sha256sum</code> (or <code>shasum</code>)</td>\n<td>The installer uses them to download and verify.</td>\n</tr>\n<tr>\n<td>A model provider key, e.g. <a href=\"https://platform.deepseek.com/api_keys\">DeepSeek</a></td>\n<td>Codewhale does nothing useful without a model.</td>\n</tr>\n<tr>\n<td><code>git</code> (recommended)</td>\n<td>Codewhale works best inside a git repository.</td>\n</tr>\n<tr>\n<td>Optional: Python 3, Node.js 20+</td>\n<td>If present, Codewhale enables its Python and JS execution tools (<code>codewhale doctor</code> lists them).</td>\n</tr>\n</tbody></table>\n</div>\n<p><strong>Which install path?</strong></p>\n<ul>\n<li><strong>Most people:</strong> the <a href=\"#2-recommended-installer-curl--sh\">recommended installer</a>.\nIt&#39;s the fastest (about 6 s here), verifies checksums, and supports\n<code>codewhale update</code>.</li>\n<li><strong>Air-gapped or security-reviewed machines:</strong>\n<a href=\"#3-manual-download-from-github-releases\">manual download</a>.</li>\n<li><strong>You already manage CLI tools with npm:</strong> <a href=\"#4-npm\">npm</a>.</li>\n<li><strong>No prebuilt binary for your platform, or you want to build it yourself:</strong>\n<a href=\"#5-cargo-and-building-from-source\">Cargo</a>.</li>\n</ul>\n<p>Pick <strong>one</strong>. Several installs on one machine end up fighting over PATH (see\n<a href=\"#13-troubleshooting\">Troubleshooting</a>).</p>\n<p><strong>Privacy note:</strong> Codewhale sends aggregate usage counts (PostHog) <strong>by\ndefault</strong>. To turn this off permanently:\n<code>codewhale config set telemetry false</code>, or export <code>CODEWHALE_TELEMETRY=0</code>,\nwhich always wins. The TUI also checks GitHub for updates at startup\n(<code>[update] check_for_updates</code> in <code>~/.codewhale/config.toml</code>).</p>\n<hr>\n<p><a id=\"recommended-official-github-releases\"></a></p>\n<h2 id=\"2-recommended-installer-curl--sh\">2. Recommended installer (<code>curl | sh</code>)</h2>\n<p><strong>Prerequisites:</strong> curl, <code>sha256sum</code> (Linux) or the built-in <code>shasum</code> (macOS),\na writable home directory. No sudo, no Node, no Rust.</p>\n"
    },
    {
      "kind": "code",
      "text": "curl -fsSL https://codewhale.net/install.sh | sh"
    },
    {
      "kind": "html",
      "text": "<p>What it does (verified):</p>\n<ul>\n<li>It detects your platform (<code>linux-x64</code>, <code>linux-arm64</code>, <code>macos-x64</code>,\n<code>macos-arm64</code>). It refuses Android/Termux and riscv64 with a clear message.</li>\n<li>It downloads <code>codewhale-&lt;platform&gt;</code>, <code>codew-&lt;platform&gt;</code> and\n<code>codewhale-artifacts-sha256.txt</code> from the latest GitHub Release, and verifies\nboth binaries against the manifest. If either doesn&#39;t match, it stops before\ninstalling anything (<code>codewhale install: checksum mismatch for …</code>).</li>\n<li>It installs <code>~/.local/bin/codewhale</code> and <code>~/.local/bin/codew</code>: two identical\n78 MB files.</li>\n<li>It <strong>never uses sudo and never edits your shell profile.</strong> It refuses to\ninstall into system or package-manager directories (<code>/usr/bin</code>,\n<code>~/.cargo/bin</code>, Homebrew, <code>node_modules</code>, <code>/nix/store</code>…) and refuses to\noverwrite a <em>different</em> existing <code>codewhale</code>.</li>\n</ul>\n<p>Expected output:</p>\n"
    },
    {
      "kind": "code",
      "text": "Installing Codewhale for linux-x64\nRelease assets: https://github.com/Hmbown/CodeWhale/releases/latest/download\nInstall dir: /home/you/.local/bin\nChecksums verified\nInstalled checksummed release commands:\n  /home/you/.local/bin/codewhale\n  /home/you/.local/bin/codew\n…\nPATH selects no codewhale command; this install is /home/you/.local/bin/codewhale"
    },
    {
      "kind": "html",
      "text": "<h3 id=\"macos-notes\">macOS notes</h3>\n<p>Re-checked on macOS 26.1, Apple silicon (<code>macos-arm64</code>), with a fresh <code>HOME</code>:</p>\n<ul>\n<li>The installer printed <code>Installing Codewhale for macos-arm64</code>, verified\nchecksums with the system tools, and installed <code>codewhale</code> and <code>codew</code>\n(64 MiB each, Mach-O arm64) in 4.3 s. Both report\n<code>codewhale 0.10.0 (1be1a703b975)</code>. They ran without a Gatekeeper prompt.</li>\n<li>When Node isn&#39;t on <code>PATH</code>, it also prints <code>Computer Use is included and needs Node.js 20 or newer on PATH.</code> The core TUI works without Node, but Computer Use\nand the JavaScript execution tool (<code>js_execution</code>) stay unavailable until Node\nis on <code>PATH</code>.</li>\n<li><code>codewhale doctor</code> behaves as on Linux (exit 0, <code>All checks complete!</code> with no\nkey, file-based secret store under <code>~/.codewhale/secrets/</code>), except that it\nreports <code>✓ sandbox available: macos-seatbelt</code>.</li>\n</ul>\n<h3 id=\"put-it-on-your-path\">Put it on your PATH</h3>\n<p>If the last lines say <code>PATH selects no codewhale command</code>, <code>~/.local/bin</code> isn&#39;t\non your PATH <strong>in this shell</strong>. On Ubuntu and Debian, <code>~/.profile</code> adds\n<code>~/.local/bin</code>, but only if the directory existed when you <em>logged in</em>. So:</p>\n<ul>\n<li>a new SSH or login shell picks it up automatically;</li>\n<li>a new terminal <strong>window</strong> on a desktop (GNOME Terminal, Ghostty, …) usually\ndoesn&#39;t, until you log out and back in. I hit\n<code>bash: codewhale: command not found</code> in Ghostty right after installing.</li>\n</ul>\n<p>Fix it once:</p>\n"
    },
    {
      "kind": "code",
      "text": "echo 'export PATH=\"$HOME/.local/bin:$PATH\"' >> ~/.bashrc   # bash\n# echo 'export PATH=\"$HOME/.local/bin:$PATH\"' >> ~/.zshrc  # zsh\n# fish_add_path ~/.local/bin                               # fish (untested on this VM)\nexport PATH=\"$HOME/.local/bin:$PATH\"; hash -r\ncommand -v codewhale codew"
    },
    {
      "kind": "html",
      "text": "<h3 id=\"options\">Options</h3>\n"
    },
    {
      "kind": "code",
      "text": "# Choose the directory (must be absolute; created if missing)\ncurl -fsSL https://codewhale.net/install.sh | CODEWHALE_INSTALL_DIR=\"$HOME/.local/codewhale/bin\" sh\n# Install a specific release\ncurl -fsSL https://codewhale.net/install.sh | CODEWHALE_VERSION=v0.9.13 sh\n# Show help\ncurl -fsSL https://codewhale.net/install.sh | sh -s -- --help"
    },
    {
      "kind": "html",
      "text": "<h3 id=\"verify\">Verify</h3>\n"
    },
    {
      "kind": "code",
      "text": "codewhale --version     # codewhale 0.10.0 (1be1a703b975)\ncodew --version         # same\ncodewhale doctor        # diagnostics; see the note in §8 about what it does NOT check"
    },
    {
      "kind": "html",
      "text": "<h3 id=\"re-running-the-installer\">Re-running the installer</h3>\n<ul>\n<li>Same version already installed: harmless. It prints\n<code>Already installed: …</code> and exits 0.</li>\n<li>Different version already installed: it <strong>refuses</strong>\n(<code>codewhale install: refusing to replace existing …/codewhale</code>), exits 1 and\nchanges nothing. It downloads ~160 MB before refusing. Use\n<a href=\"#7-updating-and-rolling-back\"><code>codewhale update</code></a> instead.</li>\n</ul>\n<h3 id=\"upgrade--uninstall\">Upgrade / uninstall</h3>\n<ul>\n<li>Upgrade: <code>codewhale update</code> (see §7).</li>\n<li>Uninstall: <code>rm ~/.local/bin/codewhale ~/.local/bin/codew</code>, then see §12 for\ndata.</li>\n</ul>\n<hr>\n<h2 id=\"3-manual-download-from-github-releases\">3. Manual download from GitHub Releases</h2>\n<p>Use this when you want to see and verify every byte yourself. Releases:\n<a href=\"https://github.com/Hmbown/CodeWhale/releases\">https://github.com/Hmbown/CodeWhale/releases</a>. Each platform has <strong>bare\nbinaries</strong> (<code>codewhale-linux-x64</code>, <code>codew-linux-x64</code>, …) and an <strong>archive</strong>\n(<code>codewhale-linux-x64.tar.gz</code>) that holds the same two binaries plus an\n<code>install.sh</code>.</p>\n<h3 id=\"3a-bare-binaries\">3a. Bare binaries</h3>\n"
    },
    {
      "kind": "code",
      "text": "mkdir -p ~/codewhale-dl && cd ~/codewhale-dl\nbase=https://github.com/Hmbown/CodeWhale/releases/latest/download\ncurl -fsSLO \"$base/codewhale-linux-x64\"          # use linux-arm64 on ARM\ncurl -fsSLO \"$base/codew-linux-x64\"\ncurl -fsSLO \"$base/codewhale-artifacts-sha256.txt\"\nsha256sum -c codewhale-artifacts-sha256.txt --ignore-missing\n#   codew-linux-x64: OK\n#   codewhale-linux-x64: OK\nmkdir -p ~/.local/bin\ninstall -m 755 codewhale-linux-x64 ~/.local/bin/codewhale\ninstall -m 755 codew-linux-x64     ~/.local/bin/codew"
    },
    {
      "kind": "html",
      "text": "<p>Then <a href=\"#put-it-on-your-path\">put <code>~/.local/bin</code> on PATH</a> and run\n<code>codewhale --version</code>. On macOS the assets are <code>codewhale-macos-arm64</code> and\n<code>codew-macos-arm64</code> (<code>-macos-x64</code> on Intel), and the built-in <code>shasum</code> verifies\nthem (tested on macOS 26.1, Apple silicon):</p>\n"
    },
    {
      "kind": "code",
      "text": "/usr/bin/shasum -a 256 -c codewhale-artifacts-sha256.txt --ignore-missing\n#   codew-macos-arm64: OK\n#   codewhale-macos-arm64: OK"
    },
    {
      "kind": "html",
      "text": "<p>The <code>codewhale-macos-arm64.tar.gz</code> archive verifies the same way against\n<code>codewhale-bundles-sha256.txt</code>, and its <code>./install.sh</code> installs into\n<code>~/.local/bin</code> (tested).</p>\n<p>To pin a release, replace <code>latest/download</code> with <code>download/vX.Y.Z</code>, and take\nthe manifest from the same tag.</p>\n<h3 id=\"3b-archive\">3b. Archive</h3>\n"
    },
    {
      "kind": "code",
      "text": "cd \"$(mktemp -d)\"\nbase=https://github.com/Hmbown/CodeWhale/releases/latest/download\ncurl -fsSLO \"$base/codewhale-linux-x64.tar.gz\"\ncurl -fsSLO \"$base/codewhale-bundles-sha256.txt\"     # note: *bundles*, not *artifacts*\nsha256sum -c codewhale-bundles-sha256.txt --ignore-missing\n#   codewhale-linux-x64.tar.gz: OK\ntar -xzf codewhale-linux-x64.tar.gz\ncd codewhale-linux-x64 && ./install.sh               # -> ~/.local/bin; PREFIX=/some/dir ./install.sh -> /some/dir/bin"
    },
    {
      "kind": "html",
      "text": "<p>The archive&#39;s <code>install.sh</code> behaves like the website installer: no sudo, it\nleaves differing existing files alone, and it prints the same PATH hint.</p>\n<p><strong>Upgrade:</strong> <code>codewhale update</code> works for both 3a and 3b, because they&#39;re\n&quot;direct binary&quot; installs. <strong>Uninstall:</strong> delete the two files (see §12).</p>\n<hr>\n<h2 id=\"4-npm\">4. npm</h2>\n<p><strong>Prerequisites:</strong> Node.js 18+ and npm, with a <strong>global prefix you can write\nto</strong>. npm installs the registry&#39;s latest published version, never an\nunpublished source candidate.</p>\n"
    },
    {
      "kind": "code",
      "text": "npm install -g codewhale\ncodewhale --version"
    },
    {
      "kind": "html",
      "text": "<p>The package is a small wrapper. Its <code>postinstall</code> step downloads the same\n<code>codewhale</code>/<code>codew</code> release binaries, checks them against the release&#39;s SHA-256\nmanifest, and links <code>codewhale</code> and <code>codew</code> into npm&#39;s global <code>bin</code>. The whole\nthing took 6 s here.</p>\n<h3 id=\"if-you-get-eacces-permission-denied\">If you get <code>EACCES: permission denied</code></h3>\n<p>That means Node is installed system-wide (apt, <code>/usr/local</code>, <code>/opt</code>), and your\nuser can&#39;t write to its global prefix:</p>\n"
    },
    {
      "kind": "code",
      "text": "npm error code EACCES\nnpm error Error: EACCES: permission denied, mkdir '/opt/node22/lib/node_modules/codewhale'"
    },
    {
      "kind": "html",
      "text": "<p><strong>Don&#39;t use <code>sudo npm</code>.</strong> Either use a per-user Node (nvm, fnm, volta), or\npoint npm at a directory you own. I tested the second option:</p>\n"
    },
    {
      "kind": "code",
      "text": "npm config set prefix \"$HOME/.npm-global\"\necho 'export PATH=\"$HOME/.npm-global/bin:$PATH\"' >> ~/.bashrc\nexport PATH=\"$HOME/.npm-global/bin:$PATH\"\nnpm install -g codewhale\ncommand -v codewhale codew     # ~/.npm-global/bin/codewhale, ~/.npm-global/bin/codew"
    },
    {
      "kind": "html",
      "text": "<h3 id=\"notes\">Notes</h3>\n<ul>\n<li>npm hides the download progress. Add <code>--foreground-scripts</code> to see it\n(<code>codewhale: selected GitHub Releases for v0.10.0 … done.</code>). The chosen\nsource is also written to\n<code>$(npm prefix -g)/lib/node_modules/codewhale/bin/downloads/codewhale.source</code>.</li>\n<li>The package uses 157 MB on disk.</li>\n<li>On macOS 26.1 (Apple silicon, Homebrew Node 25) an install into a user-owned\nprefix (<code>npm install -g --prefix &lt;dir&gt; codewhale</code>) took 3 s and linked\n<code>codewhale</code> and <code>codew</code>, both <code>codewhale 0.10.0 (1be1a703b975)</code>.</li>\n<li><strong>Upgrade:</strong> <code>npm install -g codewhale@latest</code>. <code>codewhale update</code> refuses\non npm installs. It prints migration instructions and exits 1 with\n<code>error: The package-managed executable was not changed.</code></li>\n<li><strong>Specific version:</strong> <code>npm install -g codewhale@0.9.13</code>.</li>\n<li><strong>Uninstall:</strong> <code>npm uninstall -g codewhale</code>. This removes only the program,\nnot your data (§12).</li>\n</ul>\n<hr>\n<p><a id=\"4-install-via-cargo-any-tier-1-rust-target\"></a><a id=\"7-build-from-source\"></a></p>\n<h2 id=\"5-cargo-and-building-from-source\">5. Cargo and building from source</h2>\n<p>Use this if there&#39;s no prebuilt binary for your platform, or you want to\ncompile it yourself. One Cargo package is required:\n<code>codewhale-cli</code> installs the <code>codewhale</code> command. npm and prebuilt releases also\nexpose <code>codew</code> as a convenience name for the same compiled runtime; Cargo does\nnot create that alias, so add <code>alias codew=codewhale</code> to your shell rc if you\nwant the short name.</p>\n<h3 id=\"prerequisites-debianubuntu\">Prerequisites (Debian/Ubuntu)</h3>\n"
    },
    {
      "kind": "code",
      "text": "sudo apt-get install -y build-essential pkg-config libdbus-1-dev git\n# Rust via rustup (the distro's cargo is too old for this edition-2024 workspace)\ncurl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y\nsource \"$HOME/.cargo/env\"\nrustc --version            # the workspace declares rust-version = 1.88"
    },
    {
      "kind": "html",
      "text": "<p><code>libdbus-1-dev</code> <strong>is required</strong>. Without it the build fails after about a\nminute with:</p>\n"
    },
    {
      "kind": "code",
      "text": "error: failed to run custom build command for `libdbus-sys v0.2.7`\n  The system library `dbus-1` required by crate `libdbus-sys` was not found."
    },
    {
      "kind": "html",
      "text": "<p>Fedora/RHEL: <code>sudo dnf install -y gcc make pkgconf-pkg-config dbus-devel</code>\n<strong>(untested on this VM: Ubuntu only)</strong>.</p>\n<h3 id=\"5a-from-cratesio\">5a. From crates.io</h3>\n"
    },
    {
      "kind": "code",
      "text": "cargo install codewhale-cli --locked\ncodewhale --version"
    },
    {
      "kind": "html",
      "text": "<p>Tested result: <strong>works</strong>, with current stable Rust (1.98.1).</p>\n<ul>\n<li>It took <strong>25 min 31 s</strong> on 4 vCPU and 15 GB RAM, and pulled about 470 MB\ninto <code>~/.cargo/registry</code>.</li>\n<li>It installs one 122 MB file, <code>~/.cargo/bin/codewhale</code>. That&#39;s a normal\nglibc-linked binary, and it needs <code>libdbus-1</code> at runtime.</li>\n<li><code>codewhale --version</code> prints <code>codewhale 0.10.0</code>, with no commit hash.</li>\n<li>Headless and TUI smoke tests passed.</li>\n</ul>\n<blockquote>\n<p><strong>The docs say &quot;Rust 1.88+&quot;. That&#39;s wrong for v0.10.0.</strong> With 1.88.0 the\ninstall fails in seconds:\n<code>rustc 1.88.0 is not supported by the following package: serde-saphyr@1.3.0 requires rustc 1.89</code>.\nUse current stable (<code>rustup update stable</code>).</p>\n</blockquote>\n<h3 id=\"5b-from-a-git-checkout\">5b. From a git checkout</h3>\n"
    },
    {
      "kind": "code",
      "text": "git clone --depth 1 --branch v0.10.0 https://github.com/Hmbown/CodeWhale.git\ncd CodeWhale\ncargo install --path crates/cli --locked      # installs ~/.cargo/bin/codewhale"
    },
    {
      "kind": "html",
      "text": "<p>Things to know (observed):</p>\n<ul>\n<li>The repo contains <code>rust-toolchain.toml</code> (<code>channel = &quot;stable&quot;</code>). The first\n<code>cargo</code> command inside the checkout <strong>silently downloads the latest stable\ntoolchain</strong> (about 250 MB), whatever your default is.</li>\n<li>The workspace treats every compiler warning as an error. With Rust 1.89\nthe build <strong>fails</strong> after about 11 minutes with 8\n<code>error: this lint expectation is unfulfilled</code> errors in <code>codewhale-tui</code>. Use\nthe stable toolchain the repo selects. Don&#39;t pass an older <code>+toolchain</code>.</li>\n<li>The workspace release profile uses thin LTO. On a 4-vCPU / 15 GB VM, the\n<code>codewhale-tui</code> crate alone compiled for more than an hour, peaking at\n6–8 GB of RAM. Budget 16 GB or more, and expect this to be the slowest install\npath by far. <code>target/</code> grew past 1.2 GB.</li>\n</ul>\n<p>Tested result: <strong>works</strong> on the repo-selected stable Rust (1.98.1).</p>\n<ul>\n<li><code>Finished release profile … in 83m 12s</code>. A Nix build competed for CPU and\nmemory for most of that time, so treat it as an upper bound.</li>\n<li><code>target/</code> ended at <strong>3.1 GB</strong>. Delete it afterwards with <code>cargo clean</code>.</li>\n<li>The binary reports <code>codewhale 0.10.0 (dev)</code>, and <code>exec</code> worked.</li>\n<li>Cargo prints <code>warning: default toolchain implicitly overridden with stable-x86_64-unknown-linux-gnu by rustup toolchain file</code>, which is harmless.</li>\n<li><strong>Cargo builds provide no <code>codew</code>.</strong></li>\n</ul>\n<p><strong>Upgrade:</strong> re-run the same <code>cargo install … --force</code> (for crates.io, you can\nadd <code>--version X.Y.Z</code>). <code>codewhale update</code> refuses Cargo installs.\n<strong>Uninstall:</strong> <code>cargo uninstall codewhale-cli</code>, then §12.</p>\n<hr>\n<h2 id=\"6-homebrew-on-linux-and-nix\">6. Homebrew on Linux and Nix</h2>\n<h3 id=\"homebrew-on-linux-works-but-not-with-the-command-the-old-docs-gave\">Homebrew on Linux: works, but not with the command the old docs gave</h3>\n<p>Prerequisite: Homebrew itself. Its installer needs sudo <strong>once</strong>, to create\n<code>/home/linuxbrew/.linuxbrew</code>. Without sudo rights it stops with\n<code>Insufficient permissions to install Homebrew to &quot;/home/linuxbrew/.linuxbrew&quot;</code>.\nAsk an admin to run\n<code>sudo mkdir -p /home/linuxbrew/.linuxbrew &amp;&amp; sudo chown $USER /home/linuxbrew/.linuxbrew</code>,\nthen re-run the installer. Afterwards, add\n<code>eval &quot;$(/home/linuxbrew/.linuxbrew/bin/brew shellenv bash)&quot;</code> to <code>~/.bashrc</code>,\nas its &quot;Next steps&quot; say.</p>\n"
    },
    {
      "kind": "code",
      "text": "brew install Hmbown/deepseek-tui/codewhale      # full name: taps and trusts in one step"
    },
    {
      "kind": "html",
      "text": "<p>The two-step form (<code>brew tap Hmbown/deepseek-tui</code> then <code>brew install codewhale</code>) <strong>fails on Homebrew 7.x</strong>:</p>\n"
    },
    {
      "kind": "code",
      "text": "Error: Refusing to load formula hmbown/deepseek-tui/codewhale from untrusted tap hmbown/deepseek-tui.\nRun `brew trust --formula hmbown/deepseek-tui/codewhale` or `brew trust hmbown/deepseek-tui` to trust it."
    },
    {
      "kind": "html",
      "text": "<p>Run <code>brew trust hmbown/deepseek-tui</code> first, or use the full name above.</p>\n<p>Tested with Homebrew 7.0.6: the install took 73 s. The formula is version\n0.10.0 and downloads the official release binaries, so there&#39;s no compile. It\nprovides <strong>both</strong> <code>codewhale</code> and <code>codew</code>, and depends on <code>node</code>, which pulled\nin 31 bottles (~560 MB) on Linux.</p>\n<ul>\n<li><strong>Upgrade:</strong> <code>brew upgrade codewhale</code>. (<code>codewhale update</code> refuses, and\nsuggests migrating.)</li>\n<li><strong>Uninstall:</strong> <code>brew uninstall codewhale &amp;&amp; brew untap Hmbown/deepseek-tui</code>.\nThis also autoremoves node and the other dependencies it pulled in. Homebrew&#39;s\ndownload cache (<code>~/.cache/Homebrew</code>, ~330 MB) stays until\n<code>brew cleanup --prune=all</code>.</li>\n</ul>\n<h3 id=\"nix-partially-tested\">Nix: partially tested</h3>\n"
    },
    {
      "kind": "code",
      "text": "# flakes are still experimental; the tested setup enabled them once:\nmkdir -p ~/.config/nix\necho 'experimental-features = nix-command flakes' >> ~/.config/nix/nix.conf\nnix run github:Hmbown/CodeWhale -- --version\n# one-off alternative (untested on this VM): nix --extra-experimental-features 'nix-command flakes' run github:Hmbown/CodeWhale -- --version"
    },
    {
      "kind": "html",
      "text": "<p>Nix 2.35 installed fine; single-user mode needs <code>/nix</code> created by root once.\nFlakes resolved. What I learned before stopping:</p>\n<ul>\n<li>There&#39;s <strong>no binary cache</strong>, so this is a full source build of the\n<strong>main branch</strong>, not the v0.10.0 release. The binary reports\n<code>codewhale 0.10.0 (dev)</code>.</li>\n<li>The build step took 30 min on 4 cores. The package then runs its <strong>test\nsuite</strong> (<code>doCheck</code>), which recompiles the workspace in test mode. That took\nover 70 minutes and more than 8 GB of RAM for one rustc process, and I\nstopped it at 105 minutes. So <code>nix run</code> completing, <code>nix build</code>, and\n<code>nix profile install/remove</code> are <strong>(untested on this VM: build did not\nfinish in the time budget; the VM&#39;s proxy also required an\n<code>--override-input fenix …</code> workaround)</strong>.</li>\n<li>Nix provides only <code>codewhale</code>; there&#39;s no <code>codew</code> (it builds just the\n<code>codewhale-cli</code> package).</li>\n</ul>\n<p>Unless you already live in Nix, use §2 instead.</p>\n<hr>\n<h2 id=\"7-updating-and-rolling-back\">7. Updating and rolling back</h2>\n<p>These work for installs from §2 and §3 (direct binaries). Package-manager\ninstalls (npm, Cargo, Homebrew) must be updated with their own tool.</p>\n"
    },
    {
      "kind": "code",
      "text": "codewhale update --check\n#   Current binary: /home/you/.local/bin/codewhale\n#   Current version: v0.9.13\n#   Latest stable release: v0.10.0\n#   Update available. Run `/home/you/.local/bin/codewhale update` to install v0.10.0.\ncodewhale update\n#   Downloading codewhale-linux-x64...\n#   SHA256 checksum verified against codewhale-artifacts-sha256.txt from GitHub Releases.\n#   ✅ Successfully updated to v0.10.0!\n#   Updated binaries:\n#     - /home/you/.local/bin/codewhale (codewhale-linux-x64)\n#     - /home/you/.local/bin/codew (codewhale-linux-x64)"
    },
    {
      "kind": "html",
      "text": "<p>It updates <code>codewhale</code> <strong>and</strong> <code>codew</code> together. It took 10 s here. Run it\nagain and you get <code>Already up to date; no download needed.</code> Other options:\n<code>--beta</code> and <code>--proxy &lt;URL&gt;</code>.</p>\n<p><a id=\"roll-back-to-a-previous-release\"></a></p>\n<h3 id=\"rolling-back-eg-to-v0913\">Rolling back (e.g. to v0.9.13)</h3>\n<p><code>codewhale update</code> never downgrades, and <code>CODEWHALE_VERSION=0.9.13 codewhale update</code> just says &quot;Already up to date&quot;. To roll back, replace the files:</p>\n"
    },
    {
      "kind": "code",
      "text": "dir=\"$(dirname \"$(command -v codewhale)\")\"     # the install PATH actually selects\nrm \"$dir/codewhale\" \"$dir/codew\"\ncurl -fsSL https://codewhale.net/install.sh | CODEWHALE_VERSION=v0.9.13 CODEWHALE_INSTALL_DIR=\"$dir\" sh\nhash -r; codewhale --version      # codewhale 0.9.13 (a0b81f619b66)"
    },
    {
      "kind": "html",
      "text": "<p>Tested with both the default <code>~/.local/bin</code> and a custom\n<code>CODEWHALE_INSTALL_DIR</code>. Use it only for installer, manual or archive\ninstalls. Never point it at an npm, Cargo or Homebrew directory.</p>\n<p>Or keep both versions side by side, and put the old one first on PATH:</p>\n"
    },
    {
      "kind": "code",
      "text": "curl -fsSL https://codewhale.net/install.sh | CODEWHALE_VERSION=v0.9.13 CODEWHALE_INSTALL_DIR=\"$HOME/.local/codewhale-0.9.13\" sh\nexport PATH=\"$HOME/.local/codewhale-0.9.13:$PATH\"; hash -r"
    },
    {
      "kind": "html",
      "text": "<p>To return to the latest after an in-place rollback, run <code>codewhale update</code>.\n(Tested: 0.9.13 → 0.10.0.)</p>\n<p>npm: <code>npm install -g codewhale@0.9.13</code>. Cargo:\n<code>cargo install codewhale-cli --version 0.9.13 --locked --force</code> <strong>(untested on\nthis VM: only 0.10.0 was built)</strong>.</p>\n<hr>\n<h2 id=\"8-api-keys-and-providers\">8. API keys and providers</h2>\n<h3 id=\"where-codewhale-looks-for-a-key-first-match-wins\">Where Codewhale looks for a key (first match wins)</h3>\n<ol>\n<li><code>--api-key &lt;KEY&gt;</code> on the command line</li>\n<li><code>api_key</code> in <code>~/.codewhale/config.toml</code></li>\n<li>the secret store written by <code>codewhale auth set</code></li>\n<li>the environment variable (<code>DEEPSEEK_API_KEY</code> for DeepSeek)</li>\n</ol>\n<p>This order matters. <strong>A key in config or the secret store beats\n<code>DEEPSEEK_API_KEY</code>.</strong> If you rotate your key by exporting a new env var, an\nold stored key keeps being used. I tested this: a wrong key in <code>config.toml</code>\nplus the correct env var gives <code>Authentication Fails … ****beef is invalid</code>.</p>\n<h3 id=\"ways-to-set-a-deepseek-key-all-tested\">Ways to set a DeepSeek key (all tested)</h3>\n<p><strong>Environment variable.</strong> Good for trying it out and for CI:</p>\n"
    },
    {
      "kind": "code",
      "text": "export DEEPSEEK_API_KEY=sk-...          # add to ~/.bashrc / ~/.zshenv to persist"
    },
    {
      "kind": "html",
      "text": "<p><strong><code>auth set</code>.</strong> Saves the key for every folder:</p>\n"
    },
    {
      "kind": "code",
      "text": "codewhale auth set --provider deepseek                         # prompts: \"Enter API key for deepseek:\"\nprintf '%s\\n' \"$KEY\" | codewhale auth set --provider deepseek --api-key-stdin   # scripted\n# -> saved API key for deepseek to file-based (~/.codewhale/secrets/) (config contains metadata only)"
    },
    {
      "kind": "html",
      "text": "<p>On Linux, the key is stored in <strong>plaintext</strong> in\n<code>~/.codewhale/secrets/secrets.json</code>, with mode 0600. It is not in an OS\nkeyring. Note that in v0.10.0, <code>auth set</code> also writes\n<code>default_text_model = &quot;deepseek-v4-pro&quot;</code> into your config, switching you from\nthe default <code>deepseek-flash</code> to the pricier Pro model. Change it back with\n<code>/model</code> in the TUI, or edit <code>~/.codewhale/config.toml</code>.</p>\n<p><strong>Inside the TUI.</strong> Press <strong>F3</strong> (or type <code>/provider</code>), select DeepSeek, press\nEnter, paste the key (masked), pick a model, and confirm. This also writes the\nsecret store, and keeps <code>deepseek-flash</code>.</p>\n<p><strong>Config file.</strong> <code>~/.codewhale/config.toml</code>:</p>\n"
    },
    {
      "kind": "code",
      "text": "[providers.deepseek]\napi_key = \"sk-...\""
    },
    {
      "kind": "html",
      "text": "<h3 id=\"check-which-key-is-active\">Check which key is active</h3>\n"
    },
    {
      "kind": "code",
      "text": "codewhale auth status --provider deepseek\n#   active source: env (last4: ...xxxx)        # or: secret store / config / missing\n#   lookup order: config -> secret store -> env\ncodewhale doctor --probe-api\n#   · Testing connection...  ✓ API connection successful"
    },
    {
      "kind": "html",
      "text": "<p>Use <code>auth status</code>. Plain <code>codewhale doctor</code> does <strong>not</strong> tell you: it prints\n<code>deepseek: env_source=not inspected</code> even when the key is set, and it exits 0\neven when no key is found.</p>\n<h3 id=\"remove-a-stored-key\">Remove a stored key</h3>\n"
    },
    {
      "kind": "code",
      "text": "codewhale auth clear --provider deepseek\n#   cleared API key for deepseek from config and secret store"
    },
    {
      "kind": "html",
      "text": "<p>It doesn&#39;t unset <code>DEEPSEEK_API_KEY</code> in your shell, and it leaves the\n<code>default_text_model</code> line that <code>auth set</code> added.</p>\n<h3 id=\"other-providers\">Other providers</h3>\n<p><code>codewhale auth list</code> shows about 50 providers (OpenRouter, Anthropic, OpenAI,\nMoonshot, Ollama, …). The pattern is the same:\n<code>codewhale auth set --provider &lt;name&gt;</code>, or the provider&#39;s env var. Local models\n(Ollama, vLLM, SGLang) need no key. Only DeepSeek was tested here.</p>\n<hr>\n<p><a id=\"8-shell-completions\"></a></p>\n<h2 id=\"9-shell-completions\">9. Shell completions</h2>\n"
    },
    {
      "kind": "code",
      "text": "# bash (needs the bash-completion package)\nmkdir -p ~/.local/share/bash-completion/completions\ncodewhale completion bash > ~/.local/share/bash-completion/completions/codewhale\n\n# zsh\nmkdir -p ~/.zfunc\ncodewhale completion zsh > ~/.zfunc/_codewhale\n# in ~/.zshrc, if not already there:\n#   fpath=(~/.zfunc $fpath)\n#   autoload -Uz compinit && compinit\n\n# fish\nmkdir -p ~/.config/fish/completions\ncodewhale completion fish > ~/.config/fish/completions/codewhale.fish"
    },
    {
      "kind": "html",
      "text": "<p>Each script registers both <code>codewhale</code> and <code>codew</code>. <code>codewhale completions</code> is\nan alias. Open a new shell afterwards. Regenerate after upgrading.</p>\n<p>How well they work in v0.10.0 (tested interactively):</p>\n<ul>\n<li><strong>bash:</strong> fully works (<code>codewhale comp&lt;Tab&gt;</code>, <code>codew auth &lt;Tab&gt;&lt;Tab&gt;</code>).</li>\n<li><strong>fish:</strong> sub-commands complete with descriptions, but\n<code>codewhale completion &lt;Tab&gt;</code> offers files instead of shell names.</li>\n<li><strong>zsh:</strong> only the first word completes. After a sub-command\n(<code>codewhale auth &lt;Tab&gt;</code>), zsh wrongly lists the top-level commands again\n(same on macOS zsh 5.9, where it offers all 126 top-level entries).</li>\n</ul>\n<p>PowerShell and Elvish scripts are generated too <strong>(untested on this VM: shells\nnot installed)</strong>.</p>\n<hr>\n<h2 id=\"10-running-it\">10. Running it</h2>\n<h3 id=\"the-tui\">The TUI</h3>\n"
    },
    {
      "kind": "code",
      "text": "cd your-git-repo\ncodewhale"
    },
    {
      "kind": "html",
      "text": "<ul>\n<li>The composer is at the bottom. The footer shows the permission posture\n(<code>ask</code>), the mode (<code>work</code>) and the model (<code>DeepSeek · deepseek-flash</code>).</li>\n<li><strong>Shift+Tab</strong> cycles the permission posture: Ask → Auto-Review → Full Access.\n<strong>Tab</strong> (with an empty composer) cycles the mode: Plan → Work → Operate.</li>\n<li>In <strong>Ask</strong>, file edits in the workspace are applied and shown as a diff.\nShell commands stop at an <strong>APPROVAL</strong> prompt: <code>y</code> allow once, <code>a</code> allow for\nthis session, <code>n</code> deny, <code>Esc</code> abort the turn.</li>\n<li>Useful keys: <strong>F1</strong> help (or <code>/help</code>), <strong>Ctrl-K</strong> command palette, <strong>F3</strong>\nprovider/model picker, <strong>Ctrl-R</strong> resume a past session, <strong>Ctrl-U</strong> clear the\ninput (<strong>Ctrl-Z</strong> restores it), <strong>Ctrl-C</strong> cancel or quit, <strong>Ctrl-D</strong> quit\nwith an empty input. Full list: <a href=\"https://github.com/Hmbown/CodeWhale/blob/main/docs/KEYBINDINGS.md\">KEYBINDINGS.md</a>.</li>\n<li>On exit it prints <code>To resume this session, run codewhale resume &lt;id&gt;</code>.</li>\n</ul>\n<p>Codewhale creates a <code>.codewhale/</code> directory in your repo. Ignore its contents\nbut keep the committable <code>constitution.json</code> (these are the same patterns\n<code>/init</code> writes):</p>\n"
    },
    {
      "kind": "code",
      "text": "**/.codewhale/*\n!**/.codewhale/constitution.json"
    },
    {
      "kind": "html",
      "text": "<h3 id=\"headless-scripts-ci\">Headless (scripts, CI)</h3>\n"
    },
    {
      "kind": "code",
      "text": "codewhale exec \"Reply with exactly: pong\"               # one-shot answer, no tools\ncodewhale exec --auto \"create primes.py that prints the first 10 primes and run it\"   # tools, auto-approved\ncodewhale exec --json \"…\"                               # summary JSON (provider, model, usage, output)\ncodewhale exec --auto --output-format stream-json \"…\"   # one JSON event per line"
    },
    {
      "kind": "html",
      "text": "<p><code>--auto</code> auto-approves shell commands, so use it only in a repo or sandbox you\ntrust.</p>\n<p>Plain <code>exec</code> offers the model no tools. Only <code>--auto</code>, <code>--yolo</code>,\n<code>--allowed-tools</code> or resuming a session opens a tool surface; limits such as\n<code>--max-turns</code>, <code>--disallowed-tools</code>, <code>--sandbox</code> and the output format never\nadd tools (tool-only flags print a warning). If the provider stops a reply at\nits output limit, the model is asked to continue and the printed answer is the\nwhole reply. A plain run takes at most 8 model steps unless <code>--max-turns</code> sets\nanother limit; a reply still cut off at that limit fails the run.</p>\n<h3 id=\"resuming\">Resuming</h3>\n"
    },
    {
      "kind": "code",
      "text": "codewhale resume <session-id>     # or a unique prefix, e.g. e2525dfb\ncodewhale -c                      # continue the most recent session in this folder\ncodewhale sessions                # list saved sessions\ncodewhale exec --continue \"…\"     # headless follow-up to the latest session\ncodewhale exec --resume <id> \"…\""
    },
    {
      "kind": "html",
      "text": "<p>In v0.10.0, only <strong>TUI sessions</strong> and <strong><code>--output-format stream-json</code></strong> exec\nruns are saved. A plain <code>codewhale exec</code>/<code>exec --auto</code> run is <em>not</em> saved, so a\nfollowing <code>exec --continue</code> fails with <code>No saved sessions found for workspace</code>.</p>\n<hr>\n<h2 id=\"11-terminal-notes\">11. Terminal notes</h2>\n<h3 id=\"ghostty-tested-ghostty-131-on-linuxx11\">Ghostty (tested: Ghostty 1.3.1 on Linux/X11)</h3>\n<p>Everything I checked worked in Ghostty with its default config\n(<code>TERM=xterm-ghostty</code>, <code>COLORTERM=truecolor</code>). Screenshots are kept with the\n<a href=\"https://github.com/Hmbown/Codewhale/tree/37ecdfcc49bc68a9b0d058b97c3946e62c34bd31/docs/install-report/v0.10.0-2026-09-23/screenshots\">install receipts</a>.</p>\n<div class=\"install-guide-table\" role=\"region\" aria-label=\"Installation table 2: Check / Result\" tabindex=\"0\"><table>\n<thead>\n<tr>\n<th>Check</th>\n<th>Result</th>\n</tr>\n</thead>\n<tbody><tr>\n<td>Colours / truecolor gradient, box drawing, Unicode (✓ é 日本語)</td>\n<td>✅</td>\n</tr>\n<tr>\n<td>Window resize (1504×886 → 800×500 → back) reflows cleanly</td>\n<td>✅</td>\n</tr>\n<tr>\n<td>Mouse wheel scrolls the transcript, with a jump-to-bottom button</td>\n<td>✅</td>\n</tr>\n<tr>\n<td>Paste (<code>Ctrl+Shift+V</code>), multi-line: inserted, not sent</td>\n<td>✅</td>\n</tr>\n<tr>\n<td>F1, F3, Ctrl-K, Ctrl-R, Tab, Shift+Tab, Ctrl-U/Ctrl-Z, Ctrl-C, Ctrl-D</td>\n<td>✅</td>\n</tr>\n<tr>\n<td>Window title shows state (<code>waiting on you…</code>, <code>✓ done</code>)</td>\n<td>✅</td>\n</tr>\n<tr>\n<td>Exit restores the terminal (normal screen, cursor, no mouse-reporting garbage)</td>\n<td>✅</td>\n</tr>\n</tbody></table>\n</div>\n<p>Ghostty on Linux starts a <strong>non-login</strong> shell, so it reads <code>~/.bashrc</code> and not\n<code>~/.profile</code>. That&#39;s why you need the PATH line in <code>~/.bashrc</code> (§2).</p>\n<p>You may notice small dots and a faint label (e.g. <code>other · drift</code>) drifting\nacross empty space after a turn. That&#39;s Codewhale&#39;s decorative &quot;ambient life&quot;\nwhale, not a rendering bug.</p>\n<h3 id=\"other-terminals\">Other terminals</h3>\n<p>tmux eats <strong>F1</strong>, so use <code>/help</code> there. Some key chords (Ctrl-Shift-…, Ctrl-Tab)\nneed a terminal with an enhanced keyboard protocol; <a href=\"https://github.com/Hmbown/CodeWhale/blob/main/docs/KEYBINDINGS.md\">KEYBINDINGS.md</a> lists\nportable alternatives. Windows users should use Windows Terminal\n<strong>(untested on this VM)</strong>.</p>\n<hr>\n<h2 id=\"12-uninstalling\">12. Uninstalling</h2>\n<h3 id=\"step-1-forget-stored-keys-if-you-used-auth-set-or-f3\">Step 1: forget stored keys (if you used <code>auth set</code> or F3)</h3>\n"
    },
    {
      "kind": "code",
      "text": "codewhale auth clear --provider deepseek"
    },
    {
      "kind": "html",
      "text": "<h3 id=\"step-2-remove-the-program\">Step 2: remove the program</h3>\n<div class=\"install-guide-table\" role=\"region\" aria-label=\"Installation table 3: Installed with / Remove with\" tabindex=\"0\"><table>\n<thead>\n<tr>\n<th>Installed with</th>\n<th>Remove with</th>\n</tr>\n</thead>\n<tbody><tr>\n<td>installer (§2) or manual (§3)</td>\n<td><code>rm ~/.local/bin/codewhale ~/.local/bin/codew</code> (or your <code>CODEWHALE_INSTALL_DIR</code>)</td>\n</tr>\n<tr>\n<td>npm</td>\n<td><code>npm uninstall -g codewhale</code></td>\n</tr>\n<tr>\n<td>Cargo</td>\n<td><code>cargo uninstall codewhale-cli</code></td>\n</tr>\n<tr>\n<td>Homebrew</td>\n<td><code>brew uninstall codewhale &amp;&amp; brew untap Hmbown/deepseek-tui</code> (also removes its node dependency)</td>\n</tr>\n</tbody></table>\n</div>\n<h3 id=\"step-3-remove-data-no-uninstaller-does-this-for-you\">Step 3: remove data. No uninstaller does this for you.</h3>\n<div class=\"install-guide-table\" role=\"region\" aria-label=\"Installation table 4: Path / What it is / Size seen\" tabindex=\"0\"><table>\n<thead>\n<tr>\n<th>Path</th>\n<th>What it is</th>\n<th>Size seen</th>\n</tr>\n</thead>\n<tbody><tr>\n<td><code>~/.codewhale/</code></td>\n<td>config.toml, <strong>secrets/secrets.json (plaintext keys)</strong>, sessions/, logs/, catalog/ (model list, ~5 MB), skills/, builtin-plugins/, tasks/, automations/, crashes/, audit.log, composer history</td>\n<td>6–7 MB</td>\n</tr>\n<tr>\n<td><code>~/.deepseek/snapshots/</code></td>\n<td>v0.10.0 stores its per-turn <strong>copies of your workspaces</strong> here (a legacy path). Contains the contents of every repo you ran it in.</td>\n<td>0.2–0.6 MB here; grows with repo size</td>\n</tr>\n<tr>\n<td><code>&lt;every repo you used&gt;/.codewhale/</code></td>\n<td>per-workspace state/lock dir</td>\n<td>tiny</td>\n</tr>\n<tr>\n<td>completion files</td>\n<td><code>~/.local/share/bash-completion/completions/codewhale</code>, <code>~/.zfunc/_codewhale</code>, <code>~/.config/fish/completions/codewhale.fish</code></td>\n<td>–</td>\n</tr>\n<tr>\n<td>PATH lines you added</td>\n<td><code>~/.bashrc</code>, <code>~/.zshrc</code>, <code>~/.profile</code></td>\n<td>–</td>\n</tr>\n</tbody></table>\n</div>\n"
    },
    {
      "kind": "code",
      "text": "rm -rf ~/.codewhale ~/.deepseek/snapshots\nrmdir ~/.deepseek 2>/dev/null   # removes the parent only if it is now empty\n# per-repo dirs, e.g.:\nfind ~ -type d -name .codewhale -prune -print     # review, then delete the ones you want"
    },
    {
      "kind": "html",
      "text": "<p>Codewhale wrote nothing outside <code>$HOME</code> and the repos it was used in: no\nsystem files, services or cron jobs. (I checked every file owned by the test\nusers outside their home directories.) The commands above delete only\n<code>~/.deepseek/snapshots</code>. If you still use the older DeepSeek-TUI, the rest of\n<code>~/.deepseek</code> (its config and sessions) is left alone.</p>\n<hr>\n<h2 id=\"13-troubleshooting\">13. Troubleshooting</h2>\n<p>Every error below was hit while writing this guide.</p>\n<p><strong><code>bash: codewhale: command not found</code> right after installing.</strong>\n<code>~/.local/bin</code> isn&#39;t on PATH in this terminal. See\n<a href=\"#put-it-on-your-path\">Put it on your PATH</a>.</p>\n<p><strong><code>npm error code EACCES … permission denied, mkdir &#39;…/lib/node_modules/codewhale&#39;</code>.</strong>\nYour Node is system-owned. See <a href=\"#if-you-get-eacces-permission-denied\">§4</a>.\nDon&#39;t use sudo.</p>\n<p><strong><code>error: DeepSeek API key not found.</code> (from <code>codewhale exec</code>)</strong>\nNo key anywhere. Follow the printed steps, or see §8.</p>\n<p><strong>The TUI shows your message but never answers.</strong>\nNo key (v0.10.0 doesn&#39;t say so). Press F3 → DeepSeek → Enter → paste the key.</p>\n<p><strong><code>error: Responses API request failed … Authentication Fails, Your api key: ****dead is invalid</code>.</strong>\nThe key is wrong or revoked. Run <code>codewhale auth status --provider deepseek</code>\nto see <em>which</em> source is being used. Remember that config and the secret store\nbeat the env var. Fix with <code>codewhale auth set --provider deepseek</code>, or\n<code>codewhale auth clear --provider deepseek</code> to fall back to the env var. In the\nTUI, a bad key sends you to a &quot;Choose your model provider&quot; screen that marks\nDeepSeek <code>last check failed (authentication)</code>.</p>\n<p><strong><code>error: Network error: SSE stream request failed after HTTP/1.1 fallback: Responses API request failed. … on Windows or proxy networks, try CODEWHALE_FORCE_HTTP1=1 …</code>.</strong>\nDespite the wording, on Linux this usually just means <strong>no connection to\n<code>api.deepseek.com</code></strong>. Check with <code>curl -sI https://api.deepseek.com</code> (a <code>401</code>\nresponse is fine; it means the host is reachable). If you&#39;re behind a proxy,\nmake sure <code>HTTPS_PROXY</code> is exported. <code>codewhale doctor --probe-api</code> only says\n<code>✗ API connection failed</code> for both bad keys and network problems.</p>\n<p><strong><code>codewhale install: refusing to replace existing ~/.local/bin/codewhale</code>.</strong>\nA different version is already installed there. Run <code>codewhale update</code>, or\ndelete the two files first (§7 rollback), or install into a fresh\n<code>CODEWHALE_INSTALL_DIR</code>.</p>\n<p><strong><code>codewhale install: checksum mismatch for codew-linux-x64</code>.</strong>\nThe download was corrupted or tampered with. Nothing was installed. Retry, and\nif it repeats, don&#39;t use a mirror.</p>\n<p><strong><code>error: The package-managed executable was not changed.</code> (from <code>codewhale update</code>)</strong>\nYou installed with npm, Cargo or Homebrew. Update with that tool instead.</p>\n<p><strong><code>error: failed to run custom build command for libdbus-sys</code> (Cargo).</strong>\nRun <code>sudo apt-get install -y libdbus-1-dev pkg-config</code>.</p>\n<p><strong><code>error: No saved sessions found for workspace …</code> (from <code>exec --continue</code>).</strong>\nThe previous run was plain-text <code>exec</code>, which isn&#39;t saved. Use the TUI, or\n<code>--output-format stream-json</code>.</p>\n<p><strong>zsh completion suggests the wrong things after the first word.</strong>\nKnown v0.10.0 bug. bash and fish are fine.</p>\n<p><strong>Getting help:</strong> <code>codewhale doctor --json</code> produces a diagnostics bundle\nwithout secrets.</p>\n<hr>\n<h2 id=\"appendix-other-platforms-not-re-tested-in-this-revision\">Appendix: other platforms (not re-tested in this revision)</h2>\n<p>The sections below are carried over unchanged from the previous revision of\nthis page. They were <strong>not re-run</strong> for the v0.10.0 install test above\n(out of scope: Windows, macOS, Android/Termux, FreeBSD, mainland-China\nmirrors), apart from the macOS paths noted in <a href=\"#macos-notes\">macOS notes</a>.\nKnown contradictions with the published v0.10.0 assets, found by inspecting\nthem (<a href=\"https://github.com/Hmbown/Codewhale/blob/37ecdfcc49bc68a9b0d058b97c3946e62c34bd31/docs/install-report/v0.10.0-2026-09-23/DOC_DEFECTS.md\">details</a>, D15 and D16):</p>\n<ul>\n<li>The winget manifest in <code>packaging/winget/</code> is still at 0.9.6.</li>\n<li>v0.10.0 publishes both <code>codewhale-windows-x64.zip</code> (with an <code>install.bat</code>\nthat copies to <code>%USERPROFILE%\\bin</code>) and <code>codewhale-windows-x64-portable.zip</code>;\nthe sections below mention only the first.</li>\n<li>The standalone <code>codewhale.bat</code> launcher works only next to the x64 exe.</li>\n</ul>\n<h3 id=\"supported-platforms-and-assets\">Supported platforms and assets</h3>\n<p>The <a href=\"https://github.com/Hmbown/CodeWhale/releases/latest\">latest stable release</a>\npublishes Linux x64/arm64, macOS x64/arm64, Windows x64/arm64, and Android arm64\nassets. Artifact presence is distinct from platform qualification.\nThe table below describes the current source tree&#39;s platform and secondary\npackaging support; <code>latest</code> installation still selects the published release.\nAndroid/Termux is preview pending real-device QA. Linux ARM64 is available from\nv0.8.8 onward. Linux RISC-V prebuilts are temporarily paused because the locked\n<code>rquickjs-sys</code> dependency does not ship <code>riscv64gc-unknown-linux-gnu</code> bindings.</p>\n<div class=\"install-guide-table\" role=\"region\" aria-label=\"Installation table 5: Platform / Architecture / GitHub release asset / npm install / `cargo install`\" tabindex=\"0\"><table>\n<thead>\n<tr>\n<th>Platform</th>\n<th>Architecture</th>\n<th>GitHub release asset</th>\n<th align=\"center\">npm install</th>\n<th align=\"center\"><code>cargo install</code></th>\n</tr>\n</thead>\n<tbody><tr>\n<td>Linux</td>\n<td>x64 (x86_64)</td>\n<td><code>codewhale-linux-x64</code>, <code>codew-linux-x64</code></td>\n<td align=\"center\">✅</td>\n<td align=\"center\">✅</td>\n</tr>\n<tr>\n<td>Linux</td>\n<td>arm64</td>\n<td><code>codewhale-linux-arm64</code>, <code>codew-linux-arm64</code></td>\n<td align=\"center\">✅</td>\n<td align=\"center\">✅</td>\n</tr>\n<tr>\n<td>Android / Termux</td>\n<td>arm64 (aarch64)</td>\n<td><code>codewhale-android-arm64.tar.gz</code> (published in v0.9.12; device support is preview)</td>\n<td align=\"center\">⚠️⁴ preview</td>\n<td align=\"center\">⚠️⁴ preview</td>\n</tr>\n<tr>\n<td>Linux</td>\n<td>riscv64</td>\n<td>temporarily unsupported until upstream bindings land</td>\n<td align=\"center\">❌¹</td>\n<td align=\"center\">❌³</td>\n</tr>\n<tr>\n<td>macOS</td>\n<td>x64</td>\n<td><code>codewhale-macos-x64</code>, <code>codew-macos-x64</code></td>\n<td align=\"center\">✅</td>\n<td align=\"center\">✅</td>\n</tr>\n<tr>\n<td>macOS</td>\n<td>arm64 (M-series)</td>\n<td><code>codewhale-macos-arm64</code>, <code>codew-macos-arm64</code></td>\n<td align=\"center\">✅</td>\n<td align=\"center\">✅</td>\n</tr>\n<tr>\n<td>Windows</td>\n<td>x64</td>\n<td><code>codewhale-windows-x64.exe</code>, <code>codew-windows-x64.exe</code></td>\n<td align=\"center\">✅</td>\n<td align=\"center\">✅</td>\n</tr>\n<tr>\n<td>Windows</td>\n<td>arm64</td>\n<td><code>codewhale-windows-arm64.exe</code>, <code>codew-windows-arm64.exe</code></td>\n<td align=\"center\">✅</td>\n<td align=\"center\">✅</td>\n</tr>\n<tr>\n<td>Linux x64 or arm64 on musl (Alpine)</td>\n<td>native arch</td>\n<td>matching static Linux asset</td>\n<td align=\"center\">✅ (static)</td>\n<td align=\"center\">✅</td>\n</tr>\n<tr>\n<td>Other Linux (musl on other arches)</td>\n<td>—</td>\n<td>build from source</td>\n<td align=\"center\">❌¹</td>\n<td align=\"center\">✅²</td>\n</tr>\n<tr>\n<td>FreeBSD 14+ / OpenBSD</td>\n<td>x64, arm64</td>\n<td><code>cargo install codewhale-cli --locked</code> (no prebuilt; see § FreeBSD)</td>\n<td align=\"center\">❌</td>\n<td align=\"center\">✅²</td>\n</tr>\n</tbody></table>\n</div>\n<p>¹ The npm package will exit with a clear error and point you here.\n² Provided your toolchain can compile a recent Rust workspace; see\n  <a href=\"#5-cargo-and-building-from-source\">Build from source</a> below.\n³ RISC-V source builds currently need upstream <code>rquickjs-sys</code> RISC-V bindings or\n  a bindgen-enabled dependency build.\n⁴ The current npm wrapper recognizes Android arm64 and resolves\n  the matching <code>codewhale</code> and <code>codew</code> Android assets. npm\n  installation works only for a package version whose GitHub Release publishes\n  those matching assets. The Android/Termux path remains preview-only until the\n  real-device compile, startup, approval, file-tool, and update checks tracked\n  in #4236 and #4242 are complete.</p>\n<p>Android / Termux is not the same target as Linux arm64. Do not install the\nLinux <code>codewhale-linux-arm64</code> archive in Termux; use the Termux-specific\nAndroid archive when a release or release candidate publishes one, or build\nfrom source inside Termux.</p>\n<p>The current Linux <strong>x64 and arm64</strong> assets are <strong>static musl builds</strong>.\nThe x64 release path has used musl since v0.8.65; v0.9.6 extends the same build\nand static-launch check to arm64. These binaries have no glibc dependency and\nrun on their matching architecture across Ubuntu, Debian, RHEL/CentOS, and\nAlpine/musl. SQLite is bundled through <code>rusqlite</code>, so no separate <code>libsqlite3</code>\nruntime package is needed.</p>\n<h4 id=\"linux-arm64-portability\">Linux ARM64 portability</h4>\n<p>Linux arm64 assets before v0.9.6 were GNU libc builds and could inherit the\nUbuntu 24.04 build host&#39;s <code>GLIBC_2.39</code> floor. Ubuntu 22.04 ships glibc 2.35, so\nthose older arm64 binaries can fail with errors such as:</p>\n"
    },
    {
      "kind": "code",
      "text": "version `GLIBC_2.39' not found"
    },
    {
      "kind": "html",
      "text": "<p>The npm wrapper, <code>codewhale update</code>, and the Unix archive installer retain their\nGNU-binary preflight for older releases. The current arm64 build instead uses\n<code>aarch64-unknown-linux-musl</code>, so it has no <code>GLIBC_*</code> floor. If you are installing\nan earlier release on an older arm64 distribution, use:</p>\n"
    },
    {
      "kind": "code",
      "text": "cargo install codewhale-cli --locked   # installs `codewhale`"
    },
    {
      "kind": "html",
      "text": "<blockquote>\n<p><strong>Linux ARM64 note (v0.8.7 and earlier).</strong> v0.8.7 and earlier do <strong>not</strong>\npublish a Linux ARM64 prebuilt; users on HarmonyOS thin-and-light, Asahi\nLinux, Raspberry Pi, AWS Graviton, etc. saw <code>Unsupported architecture: arm64</code>\nfrom <code>npm i -g codewhale</code>. v0.8.8 publishes <code>codewhale-linux-arm64</code>, so a plain <code>npm i -g codewhale</code> works\non any glibc-based ARM64 Linux. If you&#39;re stuck on v0.8.7, jump to\n<a href=\"#5-cargo-and-building-from-source\">Build from source</a> — <code>cargo install</code> works fine.\nFor HarmonyOS PC and OpenHarmony cross-build setup, see\n<a href=\"https://github.com/Hmbown/CodeWhale/blob/main/docs/HarmonyOS.md\">HarmonyOS and OpenHarmony</a>.</p>\n</blockquote>\n<h3 id=\"migrating-from-npm-cargo-or-another-installation\">Migrating from npm, Cargo, or another installation</h3>\n<h4 id=\"migrating-from-npm-cargo-or-another-installation-1\">Migrating from npm, Cargo, or another installation</h4>\n<p>Package managers continue to own their files. <code>codewhale update</code> gives migration\ninstructions for npm, Cargo, Homebrew, and Omarchy instead of overwriting them.\nKnown system/package directories are also protected. A <code>CODEWHALE_INSTALL_METHOD=binary</code>\noverride cannot bypass a recognized managed path.</p>\n<p>Create a fresh destination when <code>~/.local/bin</code> is occupied or a sibling command\nhas different bytes. This leaves every existing installation in place:</p>\n"
    },
    {
      "kind": "code",
      "text": "mkdir -p \"$HOME/.local\"\ncodewhale_install_dir=\"$(mktemp -d \"$HOME/.local/codewhale-release.XXXXXX\")\"\ncurl -fsSL https://codewhale.net/install.sh | CODEWHALE_INSTALL_DIR=\"$codewhale_install_dir\" sh\n\"$codewhale_install_dir/codewhale\" --version\nexport PATH=\"$codewhale_install_dir:$PATH\"\nhash -r\ncommand -v codewhale codew\n\"$codewhale_install_dir/codewhale\" update --check"
    },
    {
      "kind": "html",
      "text": "<p>After verifying the version and command paths, keep that directory first in your\nshell profile. In PowerShell, use <code>Get-Command codewhale, codew -All</code> to inspect\nresolution; run the selected executable using its full path. A successful update\nonly changes its own install directory, so another earlier PATH entry can still\nlaunch an older copy.</p>\n<p>Modern matched <code>codewhale</code>, <code>codew</code>, and compatibility copies update from the\nsame verified bytes. Symlinks to the running binary are preserved. A different\nor unrelated sibling is named in the error and left untouched; no sibling is\nexecuted merely to guess its owner. Use the fresh-directory migration above\nfor older installs with separate dispatcher/TUI binaries.</p>\n<p>To retain a secondary package-managed install, use its manager:</p>\n"
    },
    {
      "kind": "code",
      "text": "npm install -g codewhale@latest\n# or\ncargo install codewhale-cli --locked --force"
    },
    {
      "kind": "html",
      "text": "<p>Homebrew uses <code>brew upgrade codewhale</code>; Omarchy uses <code>omarchy update</code>. These\ncommands update their own copies, so verify PATH again afterward.</p>\n<p><a id=\"android--termux-arm64\"></a></p>\n<h3 id=\"android--termux-arm64-preview\">Android / Termux arm64 (preview)</h3>\n<p>Termux runs on Android&#39;s Bionic libc and uses <code>$PREFIX</code> as its Unix prefix, so\nit needs a Termux-specific Android arm64 archive. The Linux arm64 release asset\ntargets standard Linux with musl; Android uses a distinct Rust target, so the\nLinux asset should not be used there.</p>\n<p>Install the minimum archive/runtime tools first:</p>\n"
    },
    {
      "kind": "code",
      "text": "pkg update\npkg install -y ca-certificates curl tar gzip coreutils"
    },
    {
      "kind": "html",
      "text": "<p>When the release includes <code>codewhale-android-arm64.tar.gz</code>, install it with the\narchive&#39;s bundled installer. Passing <code>PREFIX=&quot;$PREFIX&quot;</code> matters: the installer\ndefaults to <code>~/.local</code>, while Termux users normally expect commands under\n<code>$PREFIX/bin</code>.</p>\n"
    },
    {
      "kind": "code",
      "text": "cd \"$HOME\"\ncurl -L -O https://github.com/Hmbown/CodeWhale/releases/latest/download/codewhale-android-arm64.tar.gz\ncurl -L -O https://github.com/Hmbown/CodeWhale/releases/latest/download/codewhale-bundles-sha256.txt\nsha256sum -c codewhale-bundles-sha256.txt --ignore-missing\n\ntar xzf codewhale-android-arm64.tar.gz\ncd codewhale-android-arm64\nPREFIX=\"$PREFIX\" ./install.sh\nhash -r"
    },
    {
      "kind": "html",
      "text": "<p>If you are validating from source or building a release candidate locally,\ninstall the build packages before running Cargo:</p>\n"
    },
    {
      "kind": "code",
      "text": "pkg install -y rust clang pkg-config make git\ncargo install codewhale-cli --locked   # installs `codewhale`"
    },
    {
      "kind": "html",
      "text": "<p>The normal first-run setup path is implemented, but its Android interaction is\nstill part of the preview QA above. Prefer provider environment variables for\ntemporary credentials. <code>codewhale auth set</code> is available, but the Termux build\nhas no supported OS keyring integration and falls back to file-backed secrets\nby writing <code>~/.codewhale/config.toml</code> and mirroring keys to\n<code>~/.codewhale/secrets/secrets.json</code>. Both are plaintext files protected by\n<code>0600</code> permissions and are not encrypted at rest.</p>\n"
    },
    {
      "kind": "code",
      "text": "codewhale auth set --provider deepseek\ncodewhale auth status\ncodewhale doctor"
    },
    {
      "kind": "html",
      "text": "<p>Maintainers should use this repeatable smoke checklist for a Termux / Android\narm64 release candidate:</p>\n"
    },
    {
      "kind": "code",
      "text": "command -v codewhale codew\ntest -x \"$PREFIX/bin/codewhale\"\ntest -x \"$PREFIX/bin/codew\"\n\ncodewhale --version\ncodewhale doctor\ncodewhale exec --auto \"run pwd\""
    },
    {
      "kind": "html",
      "text": "<p>Known limitations:</p>\n<ul>\n<li>Commands inherit Android&#39;s per-app UID, SELinux, and seccomp protections and\nany permissions granted to Termux. Codewhale&#39;s opt-in bubblewrap\nchild-process sandbox is Linux-only and is not built on Android, so approved\ncommands receive no Codewhale-specific filesystem narrowing.</li>\n<li>The Termux build has no supported Android Keystore or desktop Secret Service\nintegration. Use <code>codewhale auth status</code> to confirm the active source and\nprefer provider environment variables when file-backed plaintext storage is\nnot acceptable.</li>\n<li>Terminal rendering varies by Android terminal app. The TUI always owns the\nalternate screen. If a terminal app cannot render the full-screen TUI,\nuse <code>codewhale exec</code> for headless runs instead.</li>\n</ul>\n<h3 id=\"china--mirror-friendly-install\">China / mirror-friendly install</h3>\n<p>When installing from mainland China, configure mirrors for both <strong>rustup</strong>\n(the Rust toolchain installer) and <strong>Cargo</strong> (the package registry) to avoid\nTLS timeouts and download failures.</p>\n<p><strong>Step 1: Install Rust via a rustup mirror</strong></p>\n"
    },
    {
      "kind": "code",
      "text": "# PowerShell\n[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12\n(New-Object Net.WebClient).DownloadFile('https://win.rustup.rs/x86_64', 'rustup-init.exe')\n\n# git-bash / msys2\nexport RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup\nexport RUSTUP_UPDATE_ROOT=https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup\n./rustup-init.exe -y --default-toolchain stable\n\n# Linux / macOS\nexport RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup\nexport RUSTUP_UPDATE_ROOT=https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup\ncurl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable"
    },
    {
      "kind": "html",
      "text": "<p>If the TUNA mirror is slow from your network, <code>rsproxy.cn</code> is another\nrustup mirror option for Linux/macOS:</p>\n"
    },
    {
      "kind": "code",
      "text": "export RUSTUP_DIST_SERVER=https://rsproxy.cn\nexport RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup\ncurl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable"
    },
    {
      "kind": "html",
      "text": "<p>The <code>RUSTUP_DIST_SERVER</code> and <code>RUSTUP_UPDATE_ROOT</code> environment variables must\nbe set <strong>before</strong> running rustup-init; the toolchain download otherwise hits\nthe same TLS handshake problem as the installer.</p>\n<p><strong>Step 2: Configure Cargo registry mirror</strong></p>\n"
    },
    {
      "kind": "code",
      "text": "# ~/.cargo/config.toml\n[source.crates-io]\nreplace-with = \"tuna\"\n\n[source.tuna]\nregistry = \"sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/\""
    },
    {
      "kind": "html",
      "text": "<p><code>rsproxy</code>, Tencent COS, and Aliyun OSS mirrors work the same way; pick whichever\nis fastest from your network.</p>\n<h3 id=\"omarchy--aur\">Omarchy / AUR</h3>\n<p>On Omarchy, install the prebuilt AUR package:</p>\n"
    },
    {
      "kind": "code",
      "text": "omarchy pkg aur add codewhale-bin\ncodewhale --version"
    },
    {
      "kind": "html",
      "text": "<p><code>codewhale-bin</code> packages the same checksum-pinned Linux release archives as the\nother binary install paths and provides both <code>codewhale</code> and <code>codew</code>. It does\nnot carry a separate Codewhale version; the existing <code>codewhale-tui</code>\ncompatibility command remains an alias to the same runtime. Package updates\narrive through <code>omarchy update</code>; the in-app updater leaves the pacman-owned\nbinary to Omarchy.</p>\n<p>The AUR update follows the matching Codewhale tag and release assets, so it may\nappear after the GitHub release while its generated <code>PKGBUILD</code> and <code>.SRCINFO</code>\nare validated. Release-maintainer instructions live in\n<a href=\"https://github.com/Hmbown/CodeWhale/blob/main/packaging/aur/README.md\"><code>packaging/aur/README.md</code></a>.</p>\n<hr>\n<h3 id=\"windows\">Windows</h3>\n<h4 id=\"windows-scoop\">Windows Scoop</h4>\n<p>The <code>codewhale</code> package is listed in Scoop&#39;s main bucket:</p>\n"
    },
    {
      "kind": "code",
      "text": "scoop update\nscoop install codewhale\ncodewhale --version"
    },
    {
      "kind": "html",
      "text": "<p>Scoop manifests are maintained outside this repository&#39;s release workflow and\ncan lag GitHub/npm/Cargo releases. Use npm or manual GitHub release downloads\nwhen you need the newest version immediately.</p>\n<h4 id=\"windows-winget-v095\">Windows winget (v0.9.5+)</h4>\n<p>Codewhale publishes a winget manifest for <code>Hmbown.CodeWhale</code> (resolves #1561).\nWinget installs only the <code>codewhale</code> + <code>codew</code> commands. GitHub Releases retain\nbyte-identical <code>codewhale-tui-*</code> filenames only for legacy updater compatibility;\nthey are not a third installed command.</p>\n"
    },
    {
      "kind": "code",
      "text": "winget install Hmbown.CodeWhale\ncodewhale --version"
    },
    {
      "kind": "html",
      "text": "<p>The manifest is at <a href=\"https://github.com/Hmbown/CodeWhale/blob/main/packaging/winget/Hmbown.CodeWhale.yaml\"><code>packaging/winget/Hmbown.CodeWhale.yaml</code></a>\n(also mirrored at <a href=\"https://github.com/Hmbown/CodeWhale/blob/main/.winget/Hmbown.CodeWhale.yaml\"><code>.winget/Hmbown.CodeWhale.yaml</code></a>) and lists both\nthe NSIS installer (<code>CodeWhaleSetup.exe</code>, per-user, adds <code>%LOCALAPPDATA%\\Programs\\CodeWhale\\bin</code> to the user PATH)\nand the portable ZIP fallback (<code>codewhale-windows-x64.zip</code> / <code>codewhale-windows-arm64.zip</code>). winget\nselects the matching architecture automatically; both install the single binary (<code>codewhale.exe</code> + <code>codew.exe</code>).\nThe zips also include <code>codewhale.bat</code>. Double-click that launcher (not the raw <code>.exe</code>) so the first\nwindow is Windows Terminal when it is installed.</p>\n<p>Update via <code>winget upgrade Hmbown.CodeWhale</code> or <code>codewhale update</code>. The winget package is\nmaintained outside this repo&#39;s release workflow and can lag GitHub/npm/Cargo releases by one\nvalidation cycle — use npm or the GitHub Release asset when you need the newest version immediately.\nIf <code>winget install</code> reports a hash mismatch, verify <code>codewhale-artifacts-sha256.txt</code> for the same\ntag and regenerate the manifest via <code>packaging/winget/generate-winget-manifest.sh</code> (see\n<a href=\"https://github.com/Hmbown/CodeWhale/blob/main/packaging/winget/README.md\"><code>packaging/winget/README.md</code></a>) before re-submitting to\n<a href=\"https://github.com/microsoft/winget-pkgs\">microsoft/winget-pkgs</a>.</p>\n<blockquote>\n<p><strong>Windows ARM64 note.</strong> The NSIS installer currently contains only the x64 binaries.\nWindows ARM64 users should install via <code>winget install Hmbown.CodeWhale</code> (ARM64 ZIP) or\n<code>npm install -g codewhale</code> under native ARM64 Node.js, or download\n<code>codewhale-windows-arm64.zip</code> directly — all paths install native ARM64 binaries.</p>\n</blockquote>\n<h4 id=\"windows-nsis-installer\">Windows NSIS Installer</h4>\n<p>A standalone NSIS-based installer is available starting with v0.8.50 for\nWindows users who prefer a traditional double-click setup (no npm, no Scoop, no\nCargo required).</p>\n<p>The NSIS installer currently contains the Windows x64 binaries. Windows ARM64\nusers should install through npm running under native ARM64 Node.js or download\n<code>codewhale-windows-arm64.zip</code> from the same release; both paths then use native\nARM64 binaries.</p>\n<p><strong>Download</strong> <code>CodeWhaleSetup.exe</code> from the\n<a href=\"https://github.com/Hmbown/CodeWhale/releases/latest\">Releases page</a>.</p>\n<p><strong>Install</strong> by double-clicking the setup executable. The installer:</p>\n<ul>\n<li>Installs <code>codewhale.exe</code> and <code>codew.exe</code> side-by-side (single binary, no <code>codewhale-tui.exe</code>) into\n<code>%LOCALAPPDATA%\\Programs\\CodeWhale\\bin</code></li>\n<li>Installs <code>codewhale.bat</code>, which prefers Windows Terminal (<code>wt.exe</code>) when it is on <code>PATH</code> and\notherwise launches the exe directly</li>\n<li>Creates a current-user Start Menu shortcut that opens that launcher, not the raw <code>.exe</code></li>\n<li>Adds the install directory to the <strong>current user</strong> <code>PATH</code></li>\n<li>Registers in Windows <strong>Apps &amp; Features</strong> for easy uninstall</li>\n</ul>\n<p>Uninstall removes the binaries, <code>codewhale.bat</code>, the Start Menu shortcut, and the user <code>PATH</code> entry.</p>\n<p><strong>Silent install</strong> (for IT admins, SCCM, Intune):</p>\n"
    },
    {
      "kind": "code",
      "text": "CodeWhaleSetup.exe /S"
    },
    {
      "kind": "html",
      "text": "<p>The installer is per-user and does not request elevation. Run silent installs in\nthe target user&#39;s context, or use a deployment tool that can run the installer\nfor each user profile that needs Codewhale.</p>\n<p>The release-built installer is currently unsigned and may trigger Windows\nSmartScreen. Verify the SHA-256 checksum from <code>codewhale-artifacts-sha256.txt</code>\nbefore deploying, and sign the installer in your internal deployment pipeline if\nyour environment requires signed application packages.</p>\n<p><strong>Build the installer yourself</strong> (requires <a href=\"https://nsis.sourceforge.io/\">NSIS</a>):</p>\n"
    },
    {
      "kind": "code",
      "text": "cd scripts\\installer\n# Place codewhale.exe and codew.exe here (single binary, no codewhale-tui.exe), then:\nmakensis /DVERSION=<version> codewhale.nsi"
    },
    {
      "kind": "html",
      "text": "<p><strong>Manual fallback</strong> — if the installer is blocked by group policy, see the\n<a href=\"https://github.com/Hmbown/CodeWhale/blob/main/docs/CLASSROOM_INSTALL.md\">CLASSROOM_INSTALL.md</a> guide for step-by-step PowerShell\ncommands.</p>\n<blockquote>\n<p><strong>Deploying to a classroom or lab?</strong> See the full\n<a href=\"https://github.com/Hmbown/CodeWhale/blob/main/docs/CLASSROOM_INSTALL.md\">Classroom Install Checklist</a> for silent install,\nAPI key provisioning, imaging notes, and troubleshooting.</p>\n</blockquote>\n<p><a id=\"freebsd\"></a></p>\n<h3 id=\"freebsd-cross-compiling-windows-source-builds\">FreeBSD, cross-compiling, Windows source builds</h3>\n<h4 id=\"freebsd-14-source-build-workaround-1097\">FreeBSD 14+ source-build workaround (#1097)</h4>\n<p>FreeBSD has no prebuilt GitHub Release asset — <code>npm install -g codewhale</code> intentionally\nfails with <code>Unsupported platform: freebsd</code> and points to Cargo. Install from source:</p>\n"
    },
    {
      "kind": "code",
      "text": "pkg install -y rust pkgconf git\ncargo install codewhale-cli --locked   # installs `codewhale`\ncodewhale --version\ncodewhale doctor"
    },
    {
      "kind": "html",
      "text": "<p>The <code>rquickjs</code> FreeBSD bindings are generated at build time via <code>bindgen</code> (see\n<code>1582ba965</code>/<code>5eb0385e8</code>). No separate <code>pkg install codewhale</code> port exists yet —\na native port is tracked as the follow-up to #1097 under <code>packaging/freebsd/</code>\n(contributions welcome). Validate with <code>cargo check --target x86_64-unknown-freebsd -p codewhale-cli --locked</code>\non the release branch; the 7×1 release matrix (Linux musl x64/arm64,\nAndroid arm64, macOS x64/arm64, Windows x64/arm64) stays 7 targets — FreeBSD is a\nsource-build target, not a prebuilt asset.</p>\n<h4 id=\"cross-compiling-from-x64-to-arm64-linux\">Cross-compiling from x64 to ARM64 Linux</h4>\n<p>The release asset uses <code>aarch64-unknown-linux-musl</code> and is built on a native ARM\nrunner. If you want to build a GNU-linked ARM64 Linux binary on an x64 Linux\nhost (e.g. for a HarmonyOS / openEuler ARM64 thin-and-light), use\n<a href=\"https://github.com/cross-rs/cross\"><code>cross</code></a>, which wraps the official Rust\ncross-targets in a Docker container:</p>\n"
    },
    {
      "kind": "code",
      "text": "# Once\nrustup target add aarch64-unknown-linux-gnu\ncargo install cross --locked\n\n# Per build\ncross build --release --target aarch64-unknown-linux-gnu -p codewhale-cli   # single binary"
    },
    {
      "kind": "html",
      "text": "<p>The resulting binary lands in\n<code>target/aarch64-unknown-linux-gnu/release/codewhale</code>. Copy it to the ARM64 host\n(e.g. via <code>scp</code>) and make it executable. This local GNU build is distinct from\nthe portable musl release asset; either executable can be copied under the\n<code>codew</code> convenience name.</p>\n<p>If you don&#39;t have Docker available, install the cross-linker directly and let\nCargo do the work:</p>\n"
    },
    {
      "kind": "code",
      "text": "sudo apt-get install -y gcc-aarch64-linux-gnu\nrustup target add aarch64-unknown-linux-gnu\n\ncat >> ~/.cargo/config.toml <<'EOF'\n[target.aarch64-unknown-linux-gnu]\nlinker = \"aarch64-linux-gnu-gcc\"\nEOF\n\ncargo build --release --target aarch64-unknown-linux-gnu -p codewhale-cli   # single binary"
    },
    {
      "kind": "html",
      "text": "<p>Producing <code>aarch64-unknown-linux-musl</code> while cross-compiling requires an\nappropriate musl cross-linker. The release workflow avoids that extra moving\npart by building and launching the musl binary on GitHub&#39;s native ARM runner.</p>\n<h4 id=\"windows-build-from-source\">Windows build from source</h4>\n<p>Building on Windows requires the <strong>MSVC C toolchain</strong> from\n<a href=\"https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022\">Visual Studio Build Tools</a>\n(the free workload-selectable installer, not the full IDE).</p>\n<p><strong>Prerequisites (Windows)</strong></p>\n<ol>\n<li>Install Visual Studio 2022 Build Tools — select the <strong>&quot;Desktop development\nwith C++&quot;</strong> workload.</li>\n<li>Install <a href=\"https://rustup.rs/\">Rust</a> 1.88+ (see the\n<a href=\"#china--mirror-friendly-install\">China mirror instructions</a> above if\ndownloading from mainland China).</li>\n<li>Install <a href=\"https://git-scm.com/download/win\">Git for Windows</a> (provides <code>git</code>\nand the <code>git-bash</code> terminal).</li>\n</ol>\n<p><strong>Recommended terminals</strong>: Windows Terminal, <code>git-bash</code>, or PowerShell.\n<code>cmd.exe</code> works but has a small buffer and limited PATH behavior.</p>\n<p><strong>Setting up the MSVC environment</strong></p>\n<p>Visual Studio Build Tools install <code>cl.exe</code> to a versioned directory but do\n<strong>not</strong> add it to <code>PATH</code> globally. You must set the environment manually or\nuse a Developer Command Prompt. The required variables are:</p>\n"
    },
    {
      "kind": "code",
      "text": "# Adjust version numbers to match your installation\n$msvc = \"C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Tools\\MSVC\\14.44.35207\"\n$sdk   = \"C:\\Program Files (x86)\\Windows Kits\\10\"\n$sdkv  = \"10.0.26100.0\"\n\n$env:INCLUDE  = \"$msvc\\include;$msvc\\atlmfc\\include;$sdk\\Include\\$sdkv\\ucrt;$sdk\\Include\\$sdkv\\um;$sdk\\Include\\$sdkv\\shared\"\n$env:LIB      = \"$msvc\\lib\\x64;$msvc\\atlmfc\\lib\\x64;$sdk\\Lib\\$sdkv\\ucrt\\x64;$sdk\\Lib\\$sdkv\\um\\x64\"\n$env:LIBPATH  = \"$msvc\\lib\\x64;$msvc\\atlmfc\\lib\\x64\"\n$env:CC       = \"$msvc\\bin\\Hostx64\\x64\\cl.exe\"\n$env:CXX      = \"$msvc\\bin\\Hostx64\\x64\\cl.exe\"\n$env:PATH     = \"$msvc\\bin\\Hostx64\\x64;$env:PATH\""
    },
    {
      "kind": "html",
      "text": "<p>Alternatively, open a <strong>&quot;Developer Command Prompt for VS 2022&quot;</strong> (available\nfrom the Start Menu after installing Build Tools), which runs <code>vcvars64.bat</code>\nto configure all of the above automatically. Then add <code>cargo</code> to <code>PATH</code> inside\nthat session and run <code>cargo build</code> from the project root.</p>\n<p><strong>Cargo registry mirror</strong> — on Windows the mirror config goes to\n<code>%USERPROFILE%\\.cargo\\config.toml</code>. See <a href=\"#china--mirror-friendly-install\">Step 2 above</a>.</p>\n<p><strong>Build</strong></p>\n"
    },
    {
      "kind": "code",
      "text": "git clone https://github.com/Hmbown/CodeWhale.git\ncd CodeWhale\nset CARGO_HTTP_CHECK_REVOKE=false   # may be needed behind some Chinese ISPs\ncargo build --release"
    },
    {
      "kind": "html",
      "text": "<p>The Cargo-built binary appears at <code>target\\release\\codewhale.exe</code>. Release\npackaging separately exposes the same executable as <code>codew.exe</code>.</p>\n<blockquote>\n<p>Prefer not to build? Install via npm, Cargo, GitHub Releases, or the CNB\nmirror — see the sections above.</p>\n</blockquote>\n<h3 id=\"older-release-and-regional-troubleshooting\">Older-release and regional troubleshooting</h3>\n<h4 id=\"unsupported-architecture-arm64-on-platform-linux\"><code>Unsupported architecture: arm64 on platform linux</code></h4>\n<p>You&#39;re on a release earlier than v0.8.8 that doesn&#39;t publish Linux ARM64\nbinaries. Use the GitHub installer in a fresh directory as described above, or use\n<code>cargo install</code> per <a href=\"#5-cargo-and-building-from-source\">Section 4</a>.</p>\n<h4 id=\"missing_companion_binary-after-upgrading-an-older-install\"><code>MISSING_COMPANION_BINARY</code> after upgrading an older install</h4>\n<p>The current single binary runs the TUI in-process and does not require a\ncompanion executable. This error identifies a stale pre-v0.9.5 dispatcher.\nUse the fresh-directory GitHub migration above, then verify the selected\n<code>codewhale</code> and <code>codew</code> paths. Do not download another separate runtime.</p>\n<h4 id=\"codewhale-update-reports-no-asset-found-for-platform-codewhale-linux-aarch64\"><code>codewhale update</code> reports <code>no asset found for platform codewhale-linux-aarch64</code></h4>\n<p>Older updaters used Rust architecture names that did not match the published\nasset names. Use the official installer in a fresh directory as described above,\nthen run the newly installed command by its full path.</p>\n<h4 id=\"npm-download-is-slow-or-times-out-from-mainland-china\">npm download is slow or times out from mainland China</h4>\n<p>On Linux x64 the npm wrapper already probes GitHub Releases and the CNB\nfirst-party checksum manifests in parallel and downloads binaries only from\nthe first source that validates. You do not need <code>CODEWHALE_USE_CNB_MIRROR=1</code>\nfor that automatic path.</p>\n<p>If both first-party sources fail, set <code>CODEWHALE_RELEASE_BASE_URL</code> to a\nmirrored release-asset directory (rsproxy, TUNA, Tencent COS, Aliyun OSS),\nor skip npm entirely and use the Cargo mirror setup in\n<a href=\"#5-cargo-and-building-from-source\">Section 4</a>. The legacy\n<code>DEEPSEEK_TUI_RELEASE_BASE_URL</code> name is still accepted. <code>CODEWHALE_USE_CNB_MIRROR=1</code>\nstill forces CNB only on Linux x64 / OpenHarmony x64.</p>\n<h4 id=\"codewhale-update-is-blocked-by-github-from-mainland-china\"><code>codewhale update</code> is blocked by GitHub from mainland China</h4>\n<p><code>codewhale update</code> prefers GitHub Releases. On supported Linux x64 targets,\na failed GitHub manifest permits the matching CNB manifest and binary fallback.\nIf GitHub metadata is also unreachable, explicitly select a known published CNB\nversion (<code>CODEWHALE_USE_CNB_MIRROR=1 CODEWHALE_VERSION=X.Y.Z codewhale update</code>)\nor a binary mirror below. Existing newer builds are kept.</p>\n<p>Building from the CNB source mirror with Cargo is a secondary option. Cargo\ninstalls its own <code>codewhale</code> command:</p>\n<p>To check the latest release without downloading or replacing binaries, run\n<code>codewhale update --check</code>.</p>\n"
    },
    {
      "kind": "code",
      "text": "cargo install --git https://cnb.cool/codewhale.net/codewhale --tag vX.Y.Z codewhale-cli --locked --force   # single binary"
    },
    {
      "kind": "html",
      "text": "<p>If you operate a binary asset mirror, <code>codewhale update</code> can use it directly:</p>\n"
    },
    {
      "kind": "code",
      "text": "CODEWHALE_RELEASE_BASE_URL=https://your-mirror.example.com/CodeWhale/vX.Y.Z/ \\\nCODEWHALE_VERSION=X.Y.Z \\\ncodewhale update"
    },
    {
      "kind": "html",
      "text": "<p>The mirror directory must contain <code>codewhale-artifacts-sha256.txt</code> and the\nplatform binaries from the GitHub release. The legacy\n<code>DEEPSEEK_TUI_RELEASE_BASE_URL</code> mirror variable remains supported as an alias.</p>\n<h3 id=\"windows-and-npm-download-troubleshooting\">Windows and npm-download troubleshooting</h3>\n<h4 id=\"windows-tls-handshake-eof-or-crypt_e_revocation_offline-from-rustup-init\">Windows: <code>TLS handshake eof</code> or <code>CRYPT_E_REVOCATION_OFFLINE</code> from <code>rustup-init</code></h4>\n<p>The TLS handshake to <code>static.rust-lang.org</code> fails from behind the GFW or\ncertain Chinese ISPs. Set the rustup mirror environment variables <strong>before</strong>\nrunning the installer:</p>\n"
    },
    {
      "kind": "code",
      "text": "# git-bash / msys2\nexport RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup\nexport RUSTUP_UPDATE_ROOT=https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup\n./rustup-init.exe -y --default-toolchain stable"
    },
    {
      "kind": "html",
      "text": "<p>If you see <code>CRYPT_E_REVOCATION_OFFLINE</code> from Cargo after Rust is installed,\nalso set <code>CARGO_HTTP_CHECK_REVOKE=false</code> during <code>cargo build</code>.</p>\n<h4 id=\"windows-msvc-compiler-clexe-not-found-during-cargo-build\">Windows: MSVC compiler (<code>cl.exe</code>) not found during <code>cargo build</code></h4>\n<p>Visual Studio Build Tools do not add <code>cl.exe</code> to the global <code>PATH</code>. Either:</p>\n<ol>\n<li>Open <strong>&quot;Developer Command Prompt for VS 2022&quot;</strong> from the Start Menu, add\n<code>%USERPROFILE%\\.cargo\\bin</code> to <code>PATH</code> in that window, and run <code>cargo build</code>\nfrom there; or</li>\n<li>Set the MSVC environment variables manually — see the\n<a href=\"#windows-build-from-source\">Windows build from source</a> section for the\nPowerShell snippet.</li>\n</ol>\n<p>Verify the compiler is reachable: <code>cl.exe /?</code> should print help text.</p>\n<h4 id=\"windows-拒绝访问-os-error-5-when-cargo-executes-build-scripts\">Windows: <code>拒绝访问 (os error 5)</code> when Cargo executes build scripts</h4>\n<p>Third-party antivirus software (Huorong, 360, Kaspersky, etc.) may block\nCargo from executing freshly-compiled build-script binaries\n(e.g. <code>libsqlite3-sys</code>, <code>aws-lc-sys</code>, <code>instability</code>). The error is\npath-agnostic — moving <code>target-dir</code> does not help.</p>\n<p><strong>Symptoms</strong>: <code>could not execute process ... build-script-build (never executed)</code></p>\n<p><strong>Workarounds</strong> (pick one):</p>\n<ol>\n<li><strong>Add the project&#39;s <code>target/</code> directory to your AV exclusions list.</strong></li>\n<li><strong>Close the antivirus software temporarily</strong> during <code>cargo build</code>.</li>\n<li><strong>Use the GitHub Release installer/archive instead</strong> — the release assets\nship prebuilt binaries and skip the Cargo build entirely\n(<a href=\"#3-manual-download-from-github-releases\">Section 6</a>).</li>\n<li><strong>Use <code>cargo install codewhale-cli --locked</code></strong> from crates.io — this\nchanges the binary path, which some AV tools treat differently.</li>\n</ol>\n<p>To verify that the build-script binary itself is valid (not corrupted), locate\nit under <code>target/debug/build/&lt;crate&gt;/build-script-build</code> and run it manually:</p>\n"
    },
    {
      "kind": "code",
      "text": "target/debug/build/libsqlite3-sys-*/build-script-build\n# If this runs but panics with \"NotPresent\" (no C compiler), the binary is\n# fine — the AV is blocking Cargo's process-spawning path specifically."
    },
    {
      "kind": "html",
      "text": "<h4 id=\"npm-binary-download-times-out\">npm binary download times out</h4>\n<p>If <code>codewhale</code> waits several seconds and prints <code>connect ETIMEDOUT</code> or\n<code>EAI_AGAIN</code> while fetching from <code>github.com</code>, the npm wrapper installed\nsuccessfully but the prebuilt binary download is blocked or unreliable on\nyour network. This download is separate from the npm registry package\ndownload. On Linux x64 the wrapper first races the small GitHub and CNB\nchecksum manifests and does not wait for a full GitHub binary to time out\nbefore using a valid CNB manifest.</p>\n<p>Use one of these paths:</p>\n<ol>\n<li><p>Set a proxy and retry:</p>\n<pre><code class=\"language-bash\">export HTTPS_PROXY=http://your-proxy:port\ncodewhale\n</code></pre>\n</li>\n<li><p>Mirror the release assets internally and set <code>CODEWHALE_RELEASE_BASE_URL</code>:</p>\n<pre><code class=\"language-bash\">export CODEWHALE_RELEASE_BASE_URL=https://your-mirror.example.com/CodeWhale/\ncodewhale\n</code></pre>\n<p>The directory must contain <code>codewhale-artifacts-sha256.txt</code> and the platform\nbinaries from the GitHub release.</p>\n</li>\n<li><p>Install via Cargo, which builds locally and does not download GitHub release\nassets. See <a href=\"#5-cargo-and-building-from-source\">Section 4</a>.</p>\n</li>\n<li><p>Download both matching <code>codewhale</code> and <code>codew</code>\nbinaries from the <a href=\"https://github.com/Hmbown/CodeWhale/releases\">Releases page</a>,\nplace them in a directory on <code>PATH</code>, and make them executable. See\n<a href=\"#3-manual-download-from-github-releases\">Section 6</a>.</p>\n</li>\n</ol>\n"
    }
  ]
} as const;
