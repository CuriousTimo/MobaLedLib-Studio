//! Compiles the MobaLedLib engine (vendor/MobaLedLib, tag v3.5.0) and FastLED (vendor/FastLED, the Jueff fork
//! at a55f336, the version the Excel tool installs) for the computer, with FastLED's stub platform and the
//! Arduino stand-in in csrc/shim.

use std::path::{Path, PathBuf};

fn main() {
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let mll = dir.join("vendor/MobaLedLib/src");
    let fastled = dir.join("vendor/FastLED/src");
    for p in [&mll, &fastled] {
        if !p.is_dir() {
            panic!("{} is missing: run `git submodule update --init`", p.display());
        }
    }

    let mut b = cc::Build::new();
    b.cpp(true)
        .warnings(false)
        .include(dir.join("csrc/shim"))
        .include(&mll)
        .include(&fastled)
        .define("FASTLED_STUB_IMPL", None)
        .define("FASTLED_NO_PINMAP", None)
        .file(dir.join("csrc/sim.cpp"));
    for f in fastled_sources(&fastled) {
        b.file(f);
    }

    let target = std::env::var("TARGET").unwrap();
    if target.contains("msvc") {
        // FastLED doesn't compile with Visual Studio's cl; clang-cl does. The clang-cl that comes with
        // Visual Studio (VC\Tools\Llvm) matches its C++ library; another one may be too old for it.
        b.compiler(vs_clang_cl(&target)).flag("/std:c++17").flag("/EHsc");
    } else {
        b.flag("-std=gnu++17");
    }
    b.compile("mll_sim");

    println!("cargo:rerun-if-changed=csrc");
    println!("cargo:rerun-if-changed=vendor/MobaLedLib/src");
    println!("cargo:rerun-if-changed=vendor/FastLED/src");
}

/// Visual Studio's own clang-cl: `VC\Tools\Llvm\x64\bin\clang-cl.exe` next to the cl.exe `cc` finds,
/// else `clang-cl` from the PATH.
fn vs_clang_cl(target: &str) -> PathBuf {
    if let Some(cl) = cc::windows_registry::find_tool(target, "cl.exe") {
        // …\VC\Tools\MSVC\<version>\bin\Host<arch>\<arch>\cl.exe
        if let Some(tools) = cl.path().ancestors().nth(5) {
            let clang = tools.join("Llvm").join("x64").join("bin").join("clang-cl.exe");
            if clang.is_file() {
                return clang;
            }
        }
    }
    PathBuf::from("clang-cl")
}

/// FastLED's core sources: src/*.cpp and src/fl/*.cpp, without the stub main() and the file system (it needs
/// the video effects). The hardware platforms are not compiled; the stub platform's millis() and delay() come
/// from csrc/sim.cpp.
fn fastled_sources(src: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for d in [src.to_path_buf(), src.join("fl")] {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if name.ends_with(".cpp") && name != "stub_main.cpp" && name != "file_system.cpp" {
                v.push(p);
            }
        }
    }
    v.sort();
    v
}
