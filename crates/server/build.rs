//! Stages the built web UI so it can be embedded in the binary.
//!
//! If the UI has not been built, a placeholder page is staged instead, so the
//! server always compiles from a fresh checkout.

use std::path::Path;
use std::time::SystemTime;
use std::{env, fs, io};

const PLACEHOLDER: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Binge Server</title>
    <style>
      body { font: 16px/1.5 system-ui, sans-serif; background: #121416; color: #f4f1eb;
             display: grid; place-items: center; min-height: 100vh; margin: 0; }
      main { max-width: 34rem; padding: 2rem; }
      code { background: #282d32; padding: 0.15rem 0.4rem; border-radius: 4px; }
    </style>
  </head>
  <body>
    <main>
      <h1>Binge Server is running</h1>
      <p>The web UI was not built into this binary. Build it with
         <code>scripts/build.sh</code>, or run <code>scripts/dev.sh</code>
         while developing.</p>
    </main>
  </body>
</html>
"#;

/// Folders in `web/` that tools write. Everything else there is something the
/// UI is built from.
const GENERATED: [&str; 2] = ["node_modules", "out"];

fn main() -> io::Result<()> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
    let out_dir = env::var("OUT_DIR").expect("set by cargo");
    let release = env::var("PROFILE").is_ok_and(|profile| profile == "release");
    let web = Path::new(&manifest_dir).join("../../web");
    // `web/out` is committed and holds nothing but the built UI, so cargo can
    // watch it cheaply whether or not the UI has been built yet.
    let output = web.join("out");
    let dist = output.join("dist");
    let entry = dist.join("index.html");
    let staged = Path::new(&out_dir).join("ui");
    let built = entry.is_file();

    println!("cargo:rerun-if-changed=build.rs");
    // What tells this build from another of the same version, when the build
    // scripts say it: see `src/built.rs`.
    for name in ["BINGE_BUILD_COMMIT", "BINGE_BUILD_TIME"] {
        println!("cargo:rerun-if-env-changed={name}");
        if let Ok(value) = env::var(name) {
            println!("cargo:rustc-env={name}={value}");
        }
    }
    println!("cargo:rerun-if-changed={}", output.display());

    if staged.exists() {
        fs::remove_dir_all(&staged)?;
    }
    fs::create_dir_all(&staged)?;

    if built {
        copy_dir(&dist, &staged)?;
    } else {
        fs::write(staged.join("index.html"), PLACEHOLDER)?;
    }

    // A debug build without the UI is normal: development uses Vite. A release
    // build without it, or with an old one, is almost always a mistake.
    if release && !built {
        println!(
            "cargo:warning=the web UI is not built: this binary will show a placeholder page \
             instead. Build with scripts/build.sh."
        );
    } else if release {
        let sources = ui_sources(&web)?;
        for source in &sources {
            println!("cargo:rerun-if-changed={}", source.display());
        }
        let newest_source = sources
            .iter()
            .filter_map(|source| newest_change(source))
            .max();
        if newest_source > newest_change(&entry) {
            println!(
                "cargo:warning=the built web UI is older than its sources: this binary will \
                 embed an out-of-date UI. Build with scripts/build.sh."
            );
        }
    }
    Ok(())
}

/// Everything in `web/` the UI is built from: all of it except generated
/// folders and hidden files.
fn ui_sources(web: &Path) -> io::Result<Vec<std::path::PathBuf>> {
    let mut sources = Vec::new();
    for entry in fs::read_dir(web)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with('.') && !GENERATED.contains(&name.as_ref()) {
            sources.push(entry.path());
        }
    }
    Ok(sources)
}

/// Latest modification time of a file, or of anything inside a folder.
fn newest_change(path: &Path) -> Option<SystemTime> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_dir() {
        return metadata.modified().ok();
    }
    fs::read_dir(path)
        .ok()?
        .filter_map(|entry| newest_change(&entry.ok()?.path()))
        .max()
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
