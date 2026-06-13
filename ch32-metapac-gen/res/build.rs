use std::env;
use std::path::PathBuf;

// shared with the dump-memory-x dev tool; copied next to build.rs by ch32-metapac-gen
#[cfg(feature = "memory-x")]
#[path = "memory_x_render.rs"]
mod memory_x_render;

enum GetOneError {
    None,
    Multiple,
}

trait IteratorExt: Iterator {
    fn get_one(self) -> Result<Self::Item, GetOneError>;
}

impl<T: Iterator> IteratorExt for T {
    fn get_one(mut self) -> Result<Self::Item, GetOneError> {
        match self.next() {
            None => Err(GetOneError::None),
            Some(res) => match self.next() {
                Some(_) => Err(GetOneError::Multiple),
                None => Ok(res),
            },
        }
    }
}

fn main() {
    let crate_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());

    let chip_core_name = match env::vars()
        .map(|(a, _)| a)
        .filter(|x| x.starts_with("CARGO_FEATURE_CH32") || x.starts_with("CARGO_FEATURE_CH6"))
        .get_one()
    {
        Ok(x) => x,
        Err(GetOneError::None) => panic!("No ch32xx/ch6xx Cargo feature enabled"),
        Err(GetOneError::Multiple) => panic!("Multiple ch32xx/ch6xx Cargo features enabled"),
    }
    .strip_prefix("CARGO_FEATURE_")
    .unwrap()
    .to_ascii_lowercase()
    .replace('_', "-");

    let option = resolve_memory_option(&crate_dir, &chip_core_name);

    #[cfg(feature = "rt")]
    println!(
        "cargo:rustc-link-search={}/src/chips/{}",
        crate_dir.display(),
        chip_core_name,
    );

    #[cfg(feature = "memory-x")]
    {
        let regions_path = crate_dir
            .join("src/chips")
            .join(&chip_core_name)
            .join("memory_x")
            .join(&option)
            .join("regions");
        let regions_src = std::fs::read_to_string(&regions_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", regions_path.display(), e));
        let regions = memory_x_render::parse_regions(&regions_src);

        let split_prefixes: std::collections::BTreeSet<String> = env::vars()
            .map(|(a, _)| a)
            .filter_map(|x| {
                x.strip_prefix("CARGO_FEATURE_MEMORY_SPLIT_")
                    .map(|s| s.to_ascii_lowercase())
            })
            .collect();

        let resolved = memory_x_render::resolve_regions(&regions, &split_prefixes);
        let memory_x = memory_x_render::render_memory_x(&resolved);

        let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
        std::fs::write(out_dir.join("memory.x"), memory_x).unwrap();
        println!("cargo:rustc-link-search={}", out_dir.display());
        println!("cargo:rerun-if-changed={}", regions_path.display());
    }

    let memory_rs_path = crate_dir
        .join("src/chips")
        .join(&chip_core_name)
        .join("memory_x")
        .join(&option)
        .join("memory.rs");
    println!(
        "cargo:rustc-env=CH32_METAPAC_MEMORY_PATH={}",
        memory_rs_path.display()
    );
    println!("cargo:rerun-if-changed={}", memory_rs_path.display());

    println!(
        "cargo:rustc-env=CH32_METAPAC_PAC_PATH=chips/{}/pac.rs",
        chip_core_name
    );
    println!(
        "cargo:rustc-env=CH32_METAPAC_METADATA_PATH=chips/{}/metadata.rs",
        chip_core_name
    );

    println!("cargo:rerun-if-changed=build.rs");
}

fn resolve_memory_option(crate_dir: &std::path::Path, chip_core_name: &str) -> String {
    let explicit: Vec<String> = env::vars()
        .map(|(a, _)| a)
        .filter_map(|x| {
            x.strip_prefix("CARGO_FEATURE_MEMORY_CONFIG_")
                .map(|s| s.to_ascii_lowercase())
        })
        .collect();
    match explicit.len() {
        0 => {
            let default_path = crate_dir
                .join("src/chips")
                .join(chip_core_name)
                .join("memory_x/_default");
            std::fs::read_to_string(&default_path)
                .unwrap_or_else(|e| panic!("failed to read {}: {}", default_path.display(), e))
                .trim()
                .to_string()
        }
        1 => explicit.into_iter().next().unwrap(),
        _ => panic!(
            "Multiple `memory-config-*` features enabled: {:?}. Enable at most one.",
            explicit
        ),
    }
}
