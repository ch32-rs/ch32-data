use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Write as _};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use chiptool::generate::CommonModule;
use chiptool::{generate, ir, transform};
use proc_macro2::TokenStream;
use regex::Regex;

mod chip;
mod data;
mod memory;
use data::*;

pub struct Options {
    pub chips: Vec<String>,
    pub out_dir: PathBuf,
    pub data_dir: PathBuf,
}

pub struct Gen {
    pub(crate) opts: Options,
    pub(crate) all_peripheral_versions: HashSet<(String, String)>,
    pub(crate) metadata_dedup: HashMap<String, String>,
}

impl Gen {
    pub fn new(opts: Options) -> Self {
        Self {
            opts,
            all_peripheral_versions: HashSet::new(),
            metadata_dedup: HashMap::new(),
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
            println!("Generate Peripheral {} {}", module, version);

            let regs_path = Path::new(&self.opts.data_dir)
                .join("registers")
                .join(&format!("{}_{}.json", module, version));

            let mut ir: ir::IR = serde_json::from_reader(
                File::open(&regs_path).expect(&format!("open {}", regs_path.display())),
            )
            .unwrap();

            transform::expand_extends::ExpandExtends {}
                .run(&mut ir)
                .unwrap();

            transform::map_names(&mut ir, |k, s| match k {
                transform::NameKind::Block => *s = s.to_string(),
                transform::NameKind::Fieldset => *s = format!("regs::{}", s),
                transform::NameKind::Enum => *s = format!("vals::{}", s),
                _ => {}
            });

            transform::sort::Sort {}.run(&mut ir).unwrap();
            transform::Sanitize {}.run(&mut ir).unwrap();

            let items = generate::render(&ir, &gen_opts()).unwrap();
            let mut file = File::create(
                self.opts
                    .out_dir
                    .join("src/peripherals")
                    .join(format!("{}_{}.rs", module, version)),
            )
            .unwrap();

            // Allow a few warning
            file.write_all(
                b"#![allow(clippy::missing_safety_doc)]
                #![allow(clippy::identity_op)]
                #![allow(clippy::unnecessary_cast)]
                #![allow(clippy::erasing_op)]",
            )
            .unwrap();

            let data = items.to_string().replace("] ", "]\n");

            // Remove inner attributes like #![no_std]
            let re = Regex::new("# *! *\\[.*\\]").unwrap();
            let data = re.replace_all(&data, "");
            file.write_all(data.as_bytes()).unwrap();

            let ir = crate::data::ir::IR::from_chiptool(ir);
            let mut data = String::new();

            write!(
                &mut data,
                "
                    use crate::metadata::ir::*;
                    pub(crate) static REGISTERS: IR = {};
                ",
                stringify(&ir),
            )
            .unwrap();

            let mut file = File::create(
                self.opts
                    .out_dir
                    .join("src/registers")
                    .join(format!("{}_{}.rs", module, version)),
            )
            .unwrap();
            file.write_all(data.as_bytes()).unwrap();
        }

        // Generate Cargo.toml
        let mut contents = include_bytes!("../res/Cargo.toml").to_vec();
        for name in &chip_core_names {
            writeln!(&mut contents, "{} = []", name.to_ascii_lowercase()).unwrap();
        }
        fs::write(self.opts.out_dir.join("Cargo.toml"), contents).unwrap();

        // copy misc files
        fs::write(
            self.opts.out_dir.join("build.rs"),
            include_bytes!("../res/build.rs"),
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

