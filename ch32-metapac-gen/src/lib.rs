use std::collections::{BTreeMap, HashMap, HashSet};
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

mod data;
use data::*;

#[derive(Debug, Eq, PartialEq, Clone)]
struct Metadata<'a> {
    name: &'a str,
    family: &'a str,
    line: &'a str,
    memory: &'a [MemoryRegion],
    peripherals: &'a [Peripheral],
    interrupts: &'a [Interrupt],
    dma_channels: &'a [DmaChannel],
}

pub struct Options {
    pub chips: Vec<String>,
    pub out_dir: PathBuf,
    pub data_dir: PathBuf,
}

pub struct Gen {
    opts: Options,
    all_peripheral_versions: HashSet<(String, String)>,
    metadata_dedup: HashMap<String, String>,
}

impl Gen {
    pub fn new(opts: Options) -> Self {
        Self {
            opts,
            all_peripheral_versions: HashSet::new(),
            metadata_dedup: HashMap::new(),
        }
    }

    fn gen_chip(&mut self, chip_core_name: &str, chip: &Chip, core: &Core, core_index: usize) {
        let mut ir = ir::IR::new();

        let mut dev = ir::Device {
            // nvic_priority_bits: core.nvic_priority_bits,
            interrupts: Vec::new(),
            peripherals: Vec::new(),
            nvic_priority_bits: None, // FIXME: not used for ch32
        };

        let mut peripheral_versions: BTreeMap<String, String> = BTreeMap::new();

        let gpio_base = core
            .peripherals
            .iter()
            .find(|p| p.name == "GPIOA")
            .expect("GPIOA must exist")
            .address as u32;
        let gpio_stride = 0x400;

        for p in &core.peripherals {
            let mut ir_peri = ir::Peripheral {
                name: p.name.clone(),
                array: None,
                base_address: p.address,
                block: None,
                description: None,
                interrupts: HashMap::new(),
            };

            if let Some(bi) = &p.registers {
                if let Some(old_version) =
                    peripheral_versions.insert(bi.kind.clone(), bi.version.clone())
                {
                    if old_version != bi.version {
                        panic!(
                            "Peripheral {} has multiple versions: {} and {}",
                            bi.kind, old_version, bi.version
                        );
                    }
                }
                ir_peri.block = Some(format!("{}::{}", bi.kind, bi.block));

                if bi.kind == "gpio" {
                    assert_eq!(0, (p.address as u32 - gpio_base) % gpio_stride);
                }
            }

            dev.peripherals.push(ir_peri);
        }

        for irq in &core.interrupts {
            dev.interrupts.push(ir::Interrupt {
                name: irq.name.clone(),
                description: None,
                value: irq.number,
            });
        }

        ir.devices.insert("".to_string(), dev);

        let mut extra = format!(
            "pub fn GPIO(n: usize) -> gpio::Gpio {{
            unsafe {{ gpio::Gpio::from_ptr(({} + {}*n) as _) }}
        }}",
            gpio_base, gpio_stride,
        );

        for (module, version) in &peripheral_versions {
            self.all_peripheral_versions
                .insert((module.clone(), version.clone()));
            writeln!(
                &mut extra,
                "#[path=\"../../peripherals/{}_{}.rs\"] pub mod {};",
                module, version, module
            )
            .unwrap();
        }
        writeln!(&mut extra, "pub const CORE_INDEX: usize = {};", core_index).unwrap();

        let flash_regions: Vec<&MemoryRegion> = chip
            .memory
            .iter()
            .filter(|x| x.kind == MemoryRegionKind::Flash && x.name.starts_with("BANK_"))
            .collect();
        let first_flash = flash_regions.first().unwrap();
        let total_flash_size = flash_regions
            .iter()
            .map(|x| x.size)
            .reduce(|acc, item| acc + item)
            .unwrap();

        writeln!(
            &mut extra,
            "pub const FLASH_BASE: usize = {};",
            first_flash.address
        )
        .unwrap();
        writeln!(
            &mut extra,
            "pub const FLASH_SIZE: usize = {};",
            total_flash_size
        )
        .unwrap();

        let write_sizes: HashSet<_> = flash_regions
            .iter()
            .map(|r| r.settings.as_ref().unwrap().write_size)
            .collect();
        assert_eq!(1, write_sizes.len());
        writeln!(
            &mut extra,
            "pub const WRITE_SIZE: usize = {};",
            write_sizes.iter().next().unwrap()
        )
        .unwrap();

        // Cleanups!
        transform::sort::Sort {}.run(&mut ir).unwrap();
        transform::Sanitize {}.run(&mut ir).unwrap();

        // ==============================
        // Setup chip dir

        let chip_dir = self
            .opts
            .out_dir
            .join("src/chips")
            .join(chip_core_name.to_ascii_lowercase());
        fs::create_dir_all(&chip_dir).unwrap();

        // ==============================
        // generate pac.rs

        let data = generate::render(&ir, &gen_opts()).unwrap().to_string();
        let data = data.replace("] ", "]\n");
        // FIXME: conversion
        let data = data.replace("pub use cortex_m_rt :: interrupt ;", "");
        let data = data.replace("cortex_m :: interrupt ", "crate ");
        let data = data.replace("cortex_m", "riscv"); // FIXME

        // match riscv-rt interrupt name
        let data = data.replace(
            ".vector_table.interrupts",
            ".vector_table.external_interrupts",
        );
        let data = data.replace("__INTERRUPTS", "__EXTERNAL_INTERRUPTS");
        // trim system vector, 0 to 15
        // [Vector { _reserved : 0 } , Vector { _reserved : 0 } , Vector { _reserved : 0 }  ...
        let data = Regex::new(r#"\[(Vector \{ _reserved : 0 \} , ){16}"#)
            .unwrap()
            .replace_all(&data, "[");
        if data.contains("[Vector { _reserved : 0 }") {
            panic!("Unexpected Vector 16 {{ _reserved : 0 }}");
        }
        // Fix vector size: : [Vector; (\d+)] =
        let data = Regex::new(r#": \[Vector ; (\d+)\]"#).unwrap().replace_all(
            &data,
            |caps: &regex::Captures| {
                format!(
                    ": [Vector ; {}]",
                    caps.get(1).unwrap().as_str().parse::<usize>().unwrap() - 16
                )
            },
        );

        // Remove inner attributes like #![no_std]
        let data = Regex::new("# *! *\\[.*\\]").unwrap().replace_all(&data, "");

        let mut file = File::create(chip_dir.join("pac.rs")).unwrap();
        file.write_all(data.as_bytes()).unwrap();
        file.write_all(extra.as_bytes()).unwrap();

        let mut device_x = String::new();

        for irq in &core.interrupts {
            writeln!(&mut device_x, "PROVIDE({} = DefaultHandler);", irq.name).unwrap();
        }

        // ==============================
        // generate metadata.rs

        // (peripherals, interrupts, dma_channels) are often equal across multiple chips.
        // To reduce bloat, deduplicate them.
        let mut data = String::new();
        write!(
            &mut data,
            "
                pub(crate) static PERIPHERALS: &[Peripheral] = {};
                pub(crate) static INTERRUPTS: &[Interrupt] = {};
                pub(crate) static DMA_CHANNELS: &[DmaChannel] = {};
            ",
            stringify(&core.peripherals),
            stringify(&core.interrupts),
            stringify(&core.dma_channels),
        )
        .unwrap();

        let out_dir = self.opts.out_dir.clone();
        let n = self.metadata_dedup.len();
        let deduped_file = self.metadata_dedup.entry(data.clone()).or_insert_with(|| {
            let ir_regex = Regex::new("\":ir_for:([a-z0-9]+):\"").unwrap();
            let mut data = ir_regex.replace_all(&data, "&$1::REGISTERS").to_string();

            for (module, version) in &peripheral_versions {
                writeln!(
                    &mut data,
                    "#[path=\"../registers/{}_{}.rs\"] pub mod {};",
                    module, version, module
                )
                .unwrap();
            }

            let file = format!("metadata_{:04}.rs", n);
            let path = out_dir.join("src/chips").join(&file);
            fs::write(path, data).unwrap();

            file
        });

        let data = format!(
            "include!(\"../{}\");
            use crate::metadata::PeripheralRccKernelClock::{{Clock, Mux}};
            pub static METADATA: Metadata = Metadata {{
                name: {:?},
                family: {:?},
                line: {:?},
                memory: {},
                peripherals: PERIPHERALS,
                // nvic_priority_bits: 0,
                interrupts: INTERRUPTS,
                dma_channels: DMA_CHANNELS,
            }};",
            deduped_file,
            &chip.name,
            &chip.family,
            &chip.subfamily,
            stringify(&chip.memory),
            //&core.nvic_priority_bits,
        );

        let mut file = File::create(chip_dir.join("metadata.rs")).unwrap();
        file.write_all(data.as_bytes()).unwrap();

        // ==============================
        // generate device.x

        File::create(chip_dir.join("device.x"))
            .unwrap()
            .write_all(device_x.as_bytes())
            .unwrap();

        // ==============================
        // generate default memory.x
        gen_memory_x(&chip_dir, chip);
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

fn stringify<T: Debug>(metadata: T) -> String {
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

fn gen_memory_x(out_dir: &Path, chip: &Chip) {
    let mut memory_x = String::new();

    let flash = chip
        .memory
        .iter()
        .filter(|r| r.kind == MemoryRegionKind::Flash && r.name.starts_with("BANK_"));
    let (flash_address, flash_size) = flash
        .clone()
        .map(|r| (r.address, r.size))
        .reduce(|acc, el| (u32::min(acc.0, el.0), acc.1 + el.1))
        .unwrap();
    let ram = chip
        .memory
        .iter()
        .find(|r| r.kind == MemoryRegionKind::Ram)
        .unwrap();
    let otp = chip
        .memory
        .iter()
        .find(|r| r.kind == MemoryRegionKind::Flash && r.name == "OTP");

    write!(memory_x, "MEMORY\n{{\n").unwrap();
    writeln!(
        memory_x,
        "    FLASH : ORIGIN = 0x{:08x}, LENGTH = {:>4}K /* {} */",
        flash_address,
        flash_size / 1024,
        flash
            .map(|x| x.name.as_ref())
            .collect::<Vec<&str>>()
            .join(" + ")
    )
    .unwrap();
    writeln!(
        memory_x,
        "    RAM   : ORIGIN = 0x{:08x}, LENGTH = {:>4}K",
        ram.address,
        ram.size / 1024,
    )
    .unwrap();
    if let Some(otp) = otp {
        writeln!(
            memory_x,
            "    OTP   : ORIGIN = 0x{:08x}, LENGTH = {:>4}",
            otp.address, otp.size,
        )
        .unwrap();
    }
    write!(memory_x, "}}").unwrap();

    write!(
        memory_x,
        r#"
REGION_ALIAS("REGION_TEXT", FLASH);
REGION_ALIAS("REGION_RODATA", FLASH);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);
REGION_ALIAS("REGION_STACK", RAM);
    "#
    )
    .unwrap();

    fs::create_dir_all(out_dir.join("memory_x")).unwrap();
    let mut file = File::create(out_dir.join("memory_x").join("memory.x")).unwrap();
    file.write_all(memory_x.as_bytes()).unwrap();
}
