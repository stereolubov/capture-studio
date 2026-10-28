//! Компиляция HLSL → DXBC при сборке (§18.1).
//!
//! Если `fxc.exe` из Windows SDK найден — шейдеры пересобираются в `OUT_DIR`.
//! Иначе используется байт-код из `shaders/compiled/` (хранится в репозитории),
//! чтобы проект собирался без SDK.

use std::path::{Path, PathBuf};
use std::process::Command;

/// (файл, точка входа, профиль, имя результата)
const SHADERS: &[(&str, &str, &str, &str)] = &[
    ("egui.hlsl", "vs_main", "vs_4_0", "egui_vs.cso"),
    ("egui.hlsl", "ps_main", "ps_4_0", "egui_ps.cso"),
    ("compose.hlsl", "vs_main", "vs_4_0", "compose_vs.cso"),
    ("compose.hlsl", "ps_main", "ps_4_0", "compose_ps.cso"),
];

fn find_fxc() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CS_FXC") {
        return Some(PathBuf::from(p));
    }
    let root = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("x64").join("fxc.exe").is_file())
        .collect();
    versions.sort();
    versions.pop().map(|p| p.join("x64").join("fxc.exe"))
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let src_dir = manifest.join("shaders");
    let fallback = src_dir.join("compiled");
    println!("cargo:rerun-if-changed=shaders");
    println!("cargo:rerun-if-env-changed=CS_FXC");

    let fxc = find_fxc();
    for (file, entry, profile, name) in SHADERS {
        let dst = out.join(name);
        let compiled = fxc.as_ref().is_some_and(|fxc| {
            Command::new(fxc)
                .args(["/nologo", "/O3", "/T", profile, "/E", entry, "/Fo"])
                .arg(&dst)
                .arg(src_dir.join(file))
                .status()
                .is_ok_and(|s| s.success())
        });
        if !compiled {
            std::fs::copy(fallback.join(name), &dst).expect("prebuilt shader bytecode missing");
        }
    }
}
