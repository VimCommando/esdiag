use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=about.hbs");
    println!("cargo:rerun-if-env-changed=ESDIAG_GENERATE_NOTICE");
    println!("cargo:rerun-if-env-changed=ESDIAG_GENERATE_SBOM");

    let notice_path = Path::new("NOTICE.txt");
    let sbom_path = Path::new("esdiag.spdx.json");
    let cargo_toml_path = Path::new("Cargo.toml");
    let about_hbs_path = Path::new("about.hbs");
    let generate_notice = env_flag("ESDIAG_GENERATE_NOTICE", true);
    let generate_sbom = env_flag("ESDIAG_GENERATE_SBOM", false);

    let should_generate_notice = if !generate_notice {
        false
    } else if !notice_path.exists() {
        true
    } else {
        let notice_mtime = notice_path.metadata().and_then(|m| m.modified()).ok();
        let cargo_mtime = cargo_toml_path.metadata().and_then(|m| m.modified()).ok();
        let about_mtime = about_hbs_path.metadata().and_then(|m| m.modified()).ok();

        match (notice_mtime, cargo_mtime, about_mtime) {
            (Some(nm), Some(cm), Some(am)) => cm > nm || am > nm,
            _ => true,
        }
    };

    let should_generate_sbom = if !generate_sbom {
        false
    } else if !sbom_path.exists() {
        true
    } else {
        let sbom_mtime = sbom_path.metadata().and_then(|m| m.modified()).ok();
        let cargo_mtime = cargo_toml_path.metadata().and_then(|m| m.modified()).ok();

        match (sbom_mtime, cargo_mtime) {
            (Some(sm), Some(cm)) => cm > sm,
            _ => true,
        }
    };

    if should_generate_notice || should_generate_sbom {
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

        if should_generate_notice {
            let output_about = Command::new(&cargo)
                .args(["about", "generate", "about.hbs"])
                .output()
                .expect("failed to execute cargo about. Is cargo-about installed?");

            if output_about.status.success() {
                std::fs::write(notice_path, output_about.stdout)
                    .expect("failed to write NOTICE.txt");
            } else {
                panic!(
                    "cargo about failed: {}",
                    String::from_utf8_lossy(&output_about.stderr)
                );
            }
        }

        if should_generate_sbom {
            let output_sbom = Command::new(&cargo)
                .args(["sbom"])
                .output()
                .expect("failed to execute cargo sbom. Is cargo-sbom installed?");

            if output_sbom.status.success() {
                std::fs::write(sbom_path, output_sbom.stdout)
                    .expect("failed to write esdiag.spdx.json");
            } else {
                panic!(
                    "cargo sbom failed: {}",
                    String::from_utf8_lossy(&output_sbom.stderr)
                );
            }
        }
    }

    #[cfg(feature = "desktop")]
    {
        let manifest_dir =
            env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR for desktop build");
        let manifest_path = Path::new(&manifest_dir);
        let desktop_dir = manifest_path.join("desktop");

        emit_rerun_if_changed(manifest_path, &desktop_dir.join("tauri.conf.json"));
        emit_rerun_if_changed(manifest_path, &desktop_dir.join("capabilities"));
        emit_rerun_if_changed(manifest_path, &desktop_dir.join("frontend-dist"));
        emit_rerun_if_changed(manifest_path, &desktop_dir.join("icons"));
        emit_rerun_if_changed(manifest_path, &desktop_dir.join("packaging"));

        tauri_build::try_build(
            tauri_build::Attributes::new()
                .capabilities_path_pattern("desktop/capabilities/**/*")
                .codegen(tauri_build::CodegenContext::new().config_path("desktop/tauri.conf.json")),
        )
        .expect("failed to build desktop Tauri context");

        sync_desktop_generated_schemas(Path::new(&manifest_dir));
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    env::var(name)
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(default)
}

fn sync_desktop_generated_schemas(repo_root: &Path) {
    let generated_dir = repo_root.join("gen").join("schemas");
    let desktop_dir = repo_root.join("desktop").join("gen").join("schemas");

    if !generated_dir.exists() {
        return;
    }

    fs::create_dir_all(&desktop_dir).expect("failed to create desktop schema directory");

    for entry in fs::read_dir(&desktop_dir).expect("failed to read desktop schema directory") {
        let entry = entry.expect("failed to read desktop schema entry");
        let destination_path = entry.path();

        if destination_path.is_file() {
            let source_path = generated_dir.join(entry.file_name());
            if !source_path.exists() {
                fs::remove_file(&destination_path)
                    .expect("failed to remove stale synced desktop schema file");
            }
        }
    }

    for entry in fs::read_dir(&generated_dir).expect("failed to read generated schema directory") {
        let entry = entry.expect("failed to read generated schema entry");
        let source_path = entry.path();

        if !entry
            .file_type()
            .expect("failed to stat generated schema entry")
            .is_file()
        {
            continue;
        }

        let destination_path = desktop_dir.join(entry.file_name());
        let source_bytes = fs::read(&source_path).expect("failed to read generated schema file");
        let needs_write = fs::read(&destination_path)
            .map(|existing| existing != source_bytes)
            .unwrap_or(true);

        if needs_write {
            fs::write(&destination_path, source_bytes)
                .expect("failed to sync generated desktop schema file");
        }
    }
}

fn emit_rerun_if_changed(repo_root: &Path, path: &Path) {
    let display_path = path.strip_prefix(repo_root).unwrap_or(path);
    println!("cargo:rerun-if-changed={}", display_path.display());

    if !path.is_dir() {
        return;
    }

    for entry in fs::read_dir(path).expect("failed to read rerun-if-changed directory") {
        let entry = entry.expect("failed to read rerun-if-changed entry");
        emit_rerun_if_changed(repo_root, &entry.path());
    }
}
