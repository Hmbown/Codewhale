use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

struct Fixture(TempDir);

impl Fixture {
    fn new() -> Self {
        let fixture = Self(tempfile::tempdir().expect("create build-script fixture"));
        let support = fixture.0.path().join("support.rs");
        std::fs::write(&support, include_str!("../src/lib.rs")).unwrap();
        checked(
            Command::new(rustc())
                .args([
                    "--edition=2024",
                    "--crate-name=codewhale_build_support",
                    "--crate-type=rlib",
                ])
                .arg(support)
                .arg("-o")
                .arg(fixture.library()),
        );
        fixture
    }

    fn library(&self) -> PathBuf {
        self.0.path().join("libcodewhale_build_support.rlib")
    }

    fn compile(&self, name: &str, source: &str, manifest: &Path) -> PathBuf {
        let source_path = self.0.path().join(format!("{name}.rs"));
        std::fs::write(&source_path, source).unwrap();
        let executable = self
            .0
            .path()
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        checked(
            Command::new(rustc())
                .args(["--edition=2024", "--extern"])
                .arg(format!(
                    "codewhale_build_support={}",
                    self.library().display()
                ))
                .arg(source_path)
                .arg("-o")
                .arg(&executable)
                .env("CARGO_MANIFEST_DIR", manifest)
                .env("CARGO_PKG_VERSION", "1.2.3"),
        );
        executable
    }
}

fn rustc() -> std::ffi::OsString {
    std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into())
}

fn checked(command: &mut Command) -> Output {
    let output = command.output().expect("execute build fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn run_script(executable: &Path, manifest: &Path, cwd: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .current_dir(cwd)
        .env("CARGO_MANIFEST_DIR", manifest)
        .env("CARGO_CFG_TARGET_OS", "linux")
        .env_remove("CODEWHALE_BUILD_SHA")
        .env_remove("DEEPSEEK_BUILD_SHA")
        .env_remove("GITHUB_SHA");
    command
}

#[test]
fn cached_build_scripts_classify_the_current_manifest_and_require_it() {
    let fixture = Fixture::new();
    for (name, source) in [
        ("cli", include_str!("../../cli/build.rs")),
        ("tui", include_str!("../../tui/build.rs")),
    ] {
        let original = fixture.0.path().join(format!("{name} original"));
        let relocated = fixture.0.path().join(format!("{name} relocated"));
        std::fs::create_dir(&original).unwrap();
        let executable = fixture.compile(name, source, &original);
        let initial = checked(&mut run_script(&executable, &original, fixture.0.path()));
        assert!(
            String::from_utf8_lossy(&initial.stdout)
                .contains("cargo:rustc-env=CODEWHALE_BUILD_VERSION=1.2.3 (dev)\n")
        );

        // Reuse the same executable after the path baked in at compilation is gone.
        std::fs::rename(&original, &relocated).unwrap();
        std::fs::write(relocated.join("Cargo.toml.orig"), "packaged source").unwrap();
        let output = checked(&mut run_script(&executable, &relocated, fixture.0.path()));
        let directives = String::from_utf8_lossy(&output.stdout);
        assert!(directives.contains("cargo:rustc-env=CODEWHALE_BUILD_VERSION=1.2.3\n"));
        assert!(directives.contains("cargo:rerun-if-env-changed=CARGO_MANIFEST_DIR\n"));

        // Do not guess the current directory or fall back to the removed checkout.
        let missing = run_script(&executable, &relocated, fixture.0.path())
            .env_remove("CARGO_MANIFEST_DIR")
            .output()
            .unwrap();
        assert!(!missing.status.success());
    }
}

#[cfg(unix)]
#[test]
fn macos_helper_compilation_follows_the_relocated_manifest() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    let original = fixture.0.path().join("original manifest");
    let relocated = fixture.0.path().join("relocated manifest");
    let source = Path::new("plugins/computer-use/src/backends/darwin-accessibility.m");
    std::fs::create_dir_all(original.join(source).parent().unwrap()).unwrap();
    std::fs::write(original.join(source), "fixture source").unwrap();
    let executable = fixture.compile("tui", include_str!("../../tui/build.rs"), &original);
    std::fs::rename(&original, &relocated).unwrap();
    let out = fixture.0.path().join("out");
    let bin = fixture.0.path().join("bin");
    std::fs::create_dir(&out).unwrap();
    std::fs::create_dir(&bin).unwrap();
    // Exercise the real build-script command arguments without requiring a macOS
    // SDK or accessing a signing identity. A missing source fails as clang did.
    for (name, script) in [
        (
            "xcrun",
            "#!/bin/sh\nset -eu\nsource=\noutput=\nwhile [ \"$#\" -gt 0 ]; do\ncase \"$1\" in *.m) source=$1 ;; -o) shift; output=$1 ;; esac\nshift\ndone\ntest -f \"$source\"\nprintf %s \"$source\" > \"$output\"\n",
        ),
        (
            "codesign",
            "#!/bin/sh\nset -eu\nfor last in \"$@\"; do :; done\ntest -s \"$last\"\n",
        ),
    ] {
        let path = bin.join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = checked(
        run_script(&executable, &relocated, fixture.0.path())
            .env("CARGO_CFG_TARGET_OS", "macos")
            .env("CARGO_CFG_TARGET_ARCH", "aarch64")
            .env("OUT_DIR", &out)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env_remove("CODEWHALE_CU_SIGN_IDENTITY"),
    );
    assert_eq!(
        std::fs::read_to_string(out.join("computer-use-accessibility")).unwrap(),
        relocated.join(source).to_string_lossy()
    );
    let directives = String::from_utf8_lossy(&output.stdout);
    assert!(directives.contains(&format!(
        "cargo:rerun-if-changed={}\n",
        relocated.join(source).display()
    )));
    assert!(!directives.contains(original.to_string_lossy().as_ref()));
}
