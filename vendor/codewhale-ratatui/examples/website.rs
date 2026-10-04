//! Export actual Ratatui previews for the Codewhale component website.
//!
//! cargo run --locked --example website -- target/website-buffers
//! python3 tools/export-website.py target/website-buffers /path/to/public/ratatui
//!
//! The website receives painted buffers, rather than a second implementation
//! of the components. Width choices redraw the same fixture at that width.

use std::{collections::BTreeSet, io, path::PathBuf};

use codewhale_ratatui::{
    gallery,
    testing::{self, Profile},
};
use serde_json::{Map, Value, json};

fn export(dir: PathBuf) -> io::Result<()> {
    let entries = gallery::entries();
    let names: BTreeSet<_> = entries.iter().map(|entry| entry.name).collect();
    let safe_name = |name: &str| {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    };
    if names.len() != entries.len() || names.iter().any(|name| !safe_name(name)) {
        return Err(io::Error::other(
            "gallery names must be unique safe basenames",
        ));
    }
    std::fs::create_dir_all(&dir)?;
    let mut manifest = Vec::with_capacity(entries.len());
    let mut count = 0;
    for entry in &entries {
        let mut previews = Map::new();
        for profile in Profile::ALL {
            let theme = profile.theme();
            let resolved = gallery::theme_for(entry, &theme);
            let mut widths = Map::new();
            for (key, width) in [
                ("native", entry.width),
                ("40", 40),
                ("80", 80),
                ("120", 120),
            ] {
                let buffer = gallery::render_at(entry, &theme, width, entry.height);
                widths.insert(key.into(), Value::String(testing::svg(&buffer, &resolved)));
                count += 1;
            }
            previews.insert(profile.name().into(), Value::Object(widths));
        }
        let value = json!({
            "name": entry.name,
            "width": entry.width,
            "height": entry.height,
            "previews": previews,
        });
        std::fs::write(
            dir.join(format!("{}.json", entry.name)),
            serde_json::to_vec(&value)?,
        )?;
        manifest.push(json!({"name": entry.name, "width": entry.width, "height": entry.height}));
    }
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!(
        "Exported {count} actual buffers for {} gallery entries to {}",
        entries.len(),
        dir.display()
    );
    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: website OUTPUT_DIRECTORY",
        ));
    }
    export(PathBuf::from(&args[0]))
}
