use std::fmt::Write;
use std::path::{Path, PathBuf};

fn main() {
    // Cargo can reuse this build-script binary across worktrees sharing a
    // target directory. Resolve the package being built at execution time.
    let manifest_dir =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    println!("cargo:rerun-if-env-changed=CARGO_MANIFEST_DIR");
    codewhale_build_support::declare_rerun_conditions(&manifest_dir);
    generate_localization(&manifest_dir);
}

fn generate_localization(manifest_dir: &Path) {
    let locales = manifest_dir.join("locales");
    println!("cargo:rerun-if-changed={}", locales.display());
    // Use the same loader as rust-i18n's macro, including flattening and
    // locale merging. Store entries in static data rather than emitting one
    // map.insert statement per translation into a huge debug-stack frame.
    let translations = rust_i18n_support::try_load_locales(
        locales.to_str().expect("UTF-8 locale path"),
        |_| false,
        true,
    )
    .expect("valid translation catalog");
    assert!(!translations.is_empty(), "translation catalog is empty");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let mut data = String::from("static LOCALES: &[(&str, Messages)] = &[\n");
    for (locale, entries) in translations {
        writeln!(data, "({locale:?}, &[").unwrap();
        for (key, value) in entries {
            writeln!(data, "({key:?}, {value:?}),").unwrap();
        }
        data.push_str("]),\n");
    }
    data.push_str("];\n");
    std::fs::write(out.join("i18n_data.rs"), data).expect("write translation data");

    let bootstrap = out.join("i18n_bootstrap");
    std::fs::create_dir_all(&bootstrap).expect("create i18n bootstrap directory");
    std::fs::write(
        out.join("i18n_init.rs"),
        format!(
            "i18n!({:?}, fallback = [\"en\"], backend = crate::localization_backend::new());\n",
            bootstrap.to_str().expect("UTF-8 build path")
        ),
    )
    .expect("write i18n bootstrap");
}
