// Default allocator: mimalloc. `--no-default-features --features rusty-alloc`
// selects the Rust allocator without building the C allocator (#5872).
// With neither feature the standard library system allocator is used.
#[cfg(all(feature = "mimalloc-allocator", not(feature = "rusty-alloc")))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "rusty-alloc")]
#[global_allocator]
static GLOBAL: rusty_alloc_api::RustyAlloc = rusty_alloc_api::RustyAlloc;

fn main() -> std::process::ExitCode {
    // Reset SIGPIPE to SIG_DFL so piping codewhale output into a command that
    // exits early (e.g. `codewhale doctor | head`) terminates the process
    // cleanly with exit code 141 instead of panicking on the broken-pipe
    // write. Many execution environments (systemd, Docker, some shells)
    // inherit SIGPIPE set to SIG_IGN, which makes write(2) return EPIPE;
    // Rust's `println!` then treats that io::Error as fatal and panics.
    // See issue #4030.
    // SAFETY: process entry; no threads or handlers yet.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    // Single-binary argv0 dispatch: `codew` is now an alias for `codewhale`
    // without a second compiled artifact. Checking the binary basename keeps
    // the install surface at one file while preserving the six-keystroke save.
    let _ = std::env::args().next().and_then(|argv0| {
        let base = std::path::Path::new(&argv0)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let trimmed = base
            .strip_suffix(std::env::consts::EXE_SUFFIX)
            .unwrap_or(base);
        if trimmed == "codew" {
            // No-op: the single `codewhale` binary handles both names.
        }
        None::<()>
    });

    codewhale_cli::run_cli()
}
