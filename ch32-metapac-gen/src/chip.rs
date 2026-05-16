use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use chiptool::{generate, ir, transform};
use regex::Regex;

use crate::data::{Arch, Chip, Core, MemoryOption};
use crate::dump::split_prefixes_from_names;
use crate::memory::{
    flash_write_size, gen_memory_files, memory_select_cfg_attrs, primary_flash_regions,
};
use crate::{gen_opts, stringify, Gen};

fn build_chiptool_ir(
    chip: &Chip,
    core: &Core,
    core_index: usize,
) -> (ir::IR, BTreeMap<String, String>, String) {
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
        writeln!(
            &mut extra,
            "#[path=\"../../peripherals/{}_{}.rs\"] pub mod {};",
            module, version, module
        )
        .unwrap();
    }
    writeln!(&mut extra, "pub const CORE_INDEX: usize = {};", core_index).unwrap();

    let flash_regions: Vec<_> = primary_flash_regions(chip).collect();
    let total_flash_size = flash_regions
        .iter()
        .map(|x| x.size)
        .reduce(|acc, item| acc + item)
        .unwrap();

    let write_sizes: std::collections::HashSet<_> = flash_regions
        .iter()
        .map(|r| flash_write_size(r).expect("flash region needs a write size"))
        .collect();
    assert_eq!(1, write_sizes.len());
    let write_size = *write_sizes.iter().next().unwrap();

    let deprecation_note =
        "use ch32_metapac::metadata::METADATA.memory (enable the `metadata` feature) instead";
    writeln!(
        &mut extra,
        "#[deprecated(note = \"{}\")]\n\
         pub const FLASH_BASE: usize = 0;",
        deprecation_note,
    )
    .unwrap();
    writeln!(
        &mut extra,
        "#[deprecated(note = \"{}\")]\n\
         pub const FLASH_SIZE: usize = {};",
        deprecation_note, total_flash_size,
    )
    .unwrap();
    writeln!(
        &mut extra,
        "#[deprecated(note = \"{}\")]\n\
         pub const WRITE_SIZE: usize = {};",
        deprecation_note, write_size,
    )
    .unwrap();

    (ir, peripheral_versions, extra)
}

fn postprocess_pac_rs(s: String, arch: Arch) -> String {
    let data = s.replace("] ", "]\n");
    // Both arches drop the cortex-m-rt re-export; bins bring their own runtime.
    let data = data.replace("pub use cortex_m_rt :: interrupt ;", "");

    let data = match arch {
        Arch::Riscv => {
            // No RISC-V InterruptNumber standard; redirect to our local trait.
            let data = data.replace("cortex_m :: interrupt ", "crate ");

            // match riscv-rt interrupt name
            let data = data.replace(
                ".vector_table.interrupts",
                ".vector_table.external_interrupts",
            );
            data.replace("__INTERRUPTS", "__EXTERNAL_INTERRUPTS")
        }
        // Cortex-M NVIC uses CMSIS IRQ numbers (WWDG = 0); shift discriminants.
        Arch::Arm => {
            let data = Regex::new(r#"([A-Z][A-Z0-9_]*) = (\d+) ,"#)
                .unwrap()
                .replace_all(&data, |caps: &regex::Captures| {
                    let n: i32 = caps[2].parse().unwrap();
                    format!("{} = {} ,", &caps[1], n - 16)
                })
                .to_string();
            Regex::new(r#"# \[doc = "(\d+) - "#)
                .unwrap()
                .replace_all(&data, |caps: &regex::Captures| {
                    let n: i32 = caps[1].parse().unwrap();
                    format!(r#"# [doc = "{} - "#, n - 16)
                })
                .to_string()
        }
    };

    // trim system vector, 0 to 15
    let data = Regex::new(r#"\[(Vector \{ _reserved : 0 \} , ){16}"#)
        .unwrap()
        .replace_all(&data, "[")
        .to_string();
    if data.contains("[Vector { _reserved : 0 }") {
        panic!("Unexpected Vector 16 {{ _reserved : 0 }}");
    }
    // Fix vector size: : [Vector; (\d+)] =
    let data = Regex::new(r#": \[Vector ; (\d+)\]"#)
        .unwrap()
        .replace_all(&data, |caps: &regex::Captures| {
            format!(
                ": [Vector ; {}]",
                caps.get(1).unwrap().as_str().parse::<usize>().unwrap() - 16
            )
        })
        .to_string();

    // Remove inner attributes like #![no_std]
    Regex::new("# *! *\\[.*\\]")
        .unwrap()
        .replace_all(&data, "")
        .to_string()
}

// Returns the set of memory variants exposed via feature flags:
//   - memory_ram_code_config → one option per SRAM_CODE_MODE config (USR_2 address+size vary)
//   - memory_sizes           → a single "default" with per-chip size overrides
//   - neither                → a single "default" with no overrides
pub(crate) fn build_memory_options(chip: &Chip) -> Vec<MemoryOption> {
    if let Some(config) = &chip.memory_ram_code_config {
        let usr1_address = chip
            .memory
            .iter()
            .find(|r| r.name == "USR_1")
            .expect("memory_ram_code_config without USR_1 should have been caught by resolver")
            .address;
        return config
            .configs
            .iter()
            .map(|c| MemoryOption {
                name: c.name.clone(),
                region_sizes: vec![
                    ("USR_1".to_string(), c.code),
                    ("USR_2".to_string(), config.total_flash - c.code),
                    ("RAM".to_string(), c.ram),
                ],
                region_addresses: vec![("USR_2".to_string(), usr1_address + c.code)],
            })
            .collect();
    }

    vec![MemoryOption {
        name: "default".to_string(),
        region_sizes: chip
            .memory_sizes
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect(),
        region_addresses: Vec::new(),
    }]
}

fn render_metadata_rs(
    chip: &Chip,
    core: &Core,
    peripheral_versions: &BTreeMap<String, String>,
    memory_options: &[MemoryOption],
    default_memory_option: &str,
    metadata_dedup: &mut HashMap<String, String>,
    out_dir: &Path,
) -> String {
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

    let n = metadata_dedup.len();
    let deduped_file = metadata_dedup.entry(data.clone()).or_insert_with(|| {
        let ir_regex = Regex::new("\":ir_for:([a-z0-9]+):\"").unwrap();
        let mut data = ir_regex.replace_all(&data, "&$1::REGISTERS").to_string();

        for (module, version) in peripheral_versions {
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

    let memory_select_attrs = memory_select_cfg_attrs(memory_options, default_memory_option);

    format!(
        "include!(\"../{}\");
            use crate::metadata::PeripheralRccKernelClock::{{Clock, Mux}};
{}            mod memory_select;
            pub static METADATA: Metadata = Metadata {{
                name: {:?},
                family: {:?},
                line: {:?},
                memory: memory_select::MEMORY,
                memory_options: {},
                default_memory_option: {:?},
                peripherals: PERIPHERALS,
                // nvic_priority_bits: 0,
                interrupts: INTERRUPTS,
                dma_channels: DMA_CHANNELS,
            }};",
        deduped_file,
        memory_select_attrs,
        &chip.name,
        &chip.family,
        &chip.subfamily,
        stringify(memory_options),
        default_memory_option,
    )
}

impl Gen {
    pub(crate) fn gen_chip(
        &mut self,
        chip_core_name: &str,
        chip: &Chip,
        core: &Core,
        core_index: usize,
    ) {
        let (mut ir, peripheral_versions, extra) = build_chiptool_ir(chip, core, core_index);

        for (module, version) in &peripheral_versions {
            self.all_peripheral_versions
                .insert((module.clone(), version.clone()));
        }

        // Cleanups!
        transform::sort::Sort {}.run(&mut ir).unwrap();
        transform::Sanitize {}.run(&mut ir).unwrap();

        let chip_dir = self
            .opts
            .out_dir
            .join("src/chips")
            .join(chip_core_name.to_ascii_lowercase());
        fs::create_dir_all(&chip_dir).unwrap();

        // pac.rs
        let rendered = generate::render(&ir, &gen_opts()).unwrap().to_string();
        let pac = postprocess_pac_rs(rendered, core.arch);
        let mut file = File::create(chip_dir.join("pac.rs")).unwrap();
        file.write_all(pac.as_bytes()).unwrap();
        file.write_all(extra.as_bytes()).unwrap();

        // device.x
        let mut device_x = String::new();
        for irq in &core.interrupts {
            writeln!(&mut device_x, "PROVIDE({} = DefaultHandler);", irq.name).unwrap();
        }
        File::create(chip_dir.join("device.x"))
            .unwrap()
            .write_all(device_x.as_bytes())
            .unwrap();

        // metadata.rs
        let memory_options = build_memory_options(chip);
        let default_memory_option = chip
            .memory_ram_code_config
            .as_ref()
            .map(|c| c.default.as_str())
            .unwrap_or("default");
        if memory_options.len() > 1 {
            for opt in &memory_options {
                self.memory_option_features.insert(opt.name.clone());
            }
        }
        for prefix in split_prefixes_from_names(chip.memory.iter().map(|r| r.name.as_str())) {
            self.memory_split_prefixes.insert(prefix);
        }
        let out_dir = self.opts.out_dir.clone();
        let metadata = render_metadata_rs(
            chip,
            core,
            &peripheral_versions,
            &memory_options,
            default_memory_option,
            &mut self.metadata_dedup,
            &out_dir,
        );
        let mut file = File::create(chip_dir.join("metadata.rs")).unwrap();
        file.write_all(metadata.as_bytes()).unwrap();

        // per-option memory.x + memory.rs under memory_x/<option>/
        gen_memory_files(&chip_dir, chip, &memory_options, default_memory_option);
    }
}
