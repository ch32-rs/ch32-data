use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Debug;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

use chiptool::generate;
use chiptool::generate::CommonModule;
use proc_macro2::TokenStream;

mod chip;
mod data;
pub mod dump;
mod memory;
pub mod memory_x_render;
mod peripheral;

use data::Chip;

pub struct Options {
    pub chips: Vec<String>,
    pub out_dir: PathBuf,
    pub data_dir: PathBuf,
}

pub struct Gen {
    pub(crate) opts: Options,
    pub(crate) all_peripheral_versions: HashSet<(String, String)>,
    pub(crate) all_nv_versions: HashSet<(String, String)>,
    pub(crate) metadata_dedup: HashMap<String, String>,
    pub(crate) memory_option_features: BTreeSet<String>,
    pub(crate) memory_split_prefixes: BTreeSet<String>,
}

impl Gen {
    pub fn new(opts: Options) -> Self {
        Self {
            opts,
            all_peripheral_versions: HashSet::new(),
            all_nv_versions: HashSet::new(),
            metadata_dedup: HashMap::new(),
            memory_option_features: BTreeSet::new(),
            memory_split_prefixes: BTreeSet::new(),
        }
    }

    fn load_chip(&mut self, name: &str) -> Chip {
        let chip_path = self
            .opts
            .data_dir
            .join("chips")
            .join(format!("{}.json", name));
        let chip = fs::read(chip_path).unwrap_or_else(|_| panic!("Could not load chip {}", name));
        serde_json::from_slice(&chip).unwrap()
    }

    pub fn gen(&mut self) {
        fs::create_dir_all(self.opts.out_dir.join("src/peripherals")).unwrap();
        fs::create_dir_all(self.opts.out_dir.join("src/registers")).unwrap();
        fs::create_dir_all(self.opts.out_dir.join("src/chips")).unwrap();

        let mut chip_core_names: Vec<String> = Vec::new();

        for chip_name in &self.opts.chips.clone() {
            println!("Generate Chip {}", chip_name);

            let mut chip = self.load_chip(chip_name);

            // Cleanup
            for core in &mut chip.cores {
                for irq in &mut core.interrupts {
                    irq.name = irq.name.to_ascii_uppercase();
                }
                for p in &mut core.peripherals {
                    for irq in &mut p.interrupts {
                        irq.interrupt = irq.interrupt.to_ascii_uppercase();
                    }

                    if let Some(registers) = &mut p.registers {
                        registers.ir = format!(":ir_for:{}:", registers.kind);
                    }
                }
            }

            for region in &mut chip.memory {
                for s in &mut region.structs {
                    s.ir = format!(":ir_for:{}:", s.kind);
                    self.all_nv_versions
                        .insert((s.kind.clone(), s.version.clone()));
                }
            }

            // Generate
            for (core_index, core) in chip.cores.iter().enumerate() {
                let chip_core_name = match chip.cores.len() {
                    1 => chip_name.clone(),
                    _ => format!("{}-{}", chip_name, core.name),
                };

                chip_core_names.push(chip_core_name.clone());
                self.gen_chip(&chip_core_name, &chip, core, core_index)
            }
        }

        for (module, version) in &self.all_peripheral_versions {
            peripheral::gen_peripheral(&self.opts.out_dir, &self.opts.data_dir, module, version);
        }

        for (module, version) in &self.all_nv_versions {
            peripheral::gen_nv(&self.opts.out_dir, &self.opts.data_dir, module, version);
        }

        // Generate Cargo.toml
        let mut contents = include_bytes!("../res/Cargo.toml").to_vec();
        for name in &chip_core_names {
            writeln!(&mut contents, "{} = []", name.to_ascii_lowercase()).unwrap();
        }
        if !self.memory_option_features.is_empty() {
            writeln!(&mut contents).unwrap();
            for name in &self.memory_option_features {
                writeln!(&mut contents, "memory-config-{} = []", name).unwrap();
            }
        }
        if !self.memory_split_prefixes.is_empty() {
            writeln!(&mut contents).unwrap();
            for prefix in &self.memory_split_prefixes {
                writeln!(&mut contents, "memory-split-{} = [\"memory-x\"]", prefix).unwrap();
            }
        }
        fs::write(self.opts.out_dir.join("Cargo.toml"), contents).unwrap();

        // copy misc files
        fs::write(
            self.opts.out_dir.join("build.rs"),
            include_bytes!("../res/build.rs"),
        )
        .unwrap();
        fs::write(
            self.opts.out_dir.join("memory_x_render.rs"),
            include_bytes!("../res/memory_x_render.rs"),
        )
        .unwrap();
        fs::write(
            self.opts.out_dir.join("src/lib.rs"),
            include_bytes!("../res/src/lib.rs"),
        )
        .unwrap();
        fs::write(
            self.opts.out_dir.join("src/common.rs"),
            chiptool::generate::COMMON_MODULE,
        )
        .unwrap();
        fs::write(
            self.opts.out_dir.join("src/metadata.rs"),
            include_bytes!("../res/src/metadata.rs"),
        )
        .unwrap();
        // drop stale single-file nv.rs from earlier layout
        let _ = fs::remove_file(self.opts.out_dir.join("src/nv.rs"));
        let nv_dir = self.opts.out_dir.join("src/nv");
        fs::create_dir_all(&nv_dir).unwrap();
        fs::write(nv_dir.join("mod.rs"), include_bytes!("../res/src/nv/mod.rs")).unwrap();
        fs::write(nv_dir.join("types.rs"), include_bytes!("../res/src/nv/types.rs")).unwrap();
        fs::write(
            nv_dir.join("descriptor.rs"),
            include_bytes!("../res/src/nv/descriptor.rs"),
        )
        .unwrap();
        fs::write(nv_dir.join("codec.rs"), include_bytes!("../res/src/nv/codec.rs")).unwrap();
        fs::write(
            nv_dir.join("lifecycle.rs"),
            include_bytes!("../res/src/nv/lifecycle.rs"),
        )
        .unwrap();
        fs::write(nv_dir.join("tests.rs"), include_bytes!("../res/src/nv/tests.rs")).unwrap();
    }
}

pub(crate) fn stringify<T: Debug>(metadata: T) -> String {
    let mut metadata = format!("{:#?}", metadata);
    if metadata.starts_with('[') {
        metadata = format!("&{}", metadata);
    }

    metadata.replace(": [", ": &[")
}

fn gen_opts() -> generate::Options {
    generate::Options {
        common_module: CommonModule::External(TokenStream::from_str("crate::common").unwrap()),
    }
}

