//! `mll`: command line tools for MobaLedLib Studio.
//!
//! ```text
//! mll extract-catalog <Prog_Generator_MobaLedLib.xlsm> <library.properties> <out.json>
//! mll generate <layout.json|file.MLL_pgf> [sheet-index] [out.h]
//! mll convert <in.json|in.MLL_pgf> <out.json|out.MLL_pgf>
//! ```

mod extract;

use std::path::Path;

use anyhow::{bail, Context, Result};
use mll_core::codegen::{self, GenOptions};
use mll_core::{pgf, Catalog, Layout};

fn load_layout(path: &Path) -> Result<Layout> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if is_pgf(path) {
        Ok(pgf::import(&bytes, &name)?)
    } else {
        Ok(Layout::from_json(&String::from_utf8(bytes)?)?)
    }
}

fn is_pgf(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("MLL_pgf"))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("extract-catalog") if args.len() == 4 => {
            let catalog = extract::extract(Path::new(&args[1]), Path::new(&args[2]))?;
            std::fs::write(&args[3], serde_json::to_string_pretty(&catalog)? + "\n")?;
            eprintln!(
                "{} macros, {} parameters, {} platforms written to {}",
                catalog.macros.len(),
                catalog.params.len(),
                catalog.platforms.len(),
                args[3]
            );
        }
        Some("generate") if args.len() >= 2 => {
            let layout = load_layout(Path::new(&args[1]))?;
            let sheet: usize = args.get(2).map(|s| s.parse()).transpose()?.unwrap_or(0);
            let catalog = Catalog::bundled();
            let opts = GenOptions { lib_version: catalog.lib_version.clone(), ..GenOptions::default() };
            match codegen::generate(&layout, sheet, &catalog, &opts) {
                Ok(g) => {
                    for w in &g.warnings {
                        eprintln!("warning: {}", w.message);
                    }
                    match args.get(3) {
                        Some(out) => std::fs::write(out, &g.header)?,
                        None => print!("{}", g.header),
                    }
                }
                Err(e) => bail!("{e}"),
            }
        }
        Some("convert") if args.len() == 3 => {
            let layout = load_layout(Path::new(&args[1]))?;
            let out = Path::new(&args[2]);
            if is_pgf(out) {
                std::fs::write(out, pgf::export(&layout))?;
            } else {
                std::fs::write(out, layout.to_json() + "\n")?;
            }
        }
        _ => {
            eprintln!("usage:");
            eprintln!("  mll extract-catalog <Prog_Generator_MobaLedLib.xlsm> <library.properties> <out.json>");
            eprintln!("  mll generate <layout.json|file.MLL_pgf> [sheet-index] [out.h]");
            eprintln!("  mll convert <in.json|in.MLL_pgf> <out.json|out.MLL_pgf>");
            std::process::exit(2);
        }
    }
    Ok(())
}
