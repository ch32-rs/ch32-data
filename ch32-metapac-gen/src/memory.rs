use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::data::{Access, Chip, MemoryOption, MemoryRegion, MemoryRegionKind, Mode};
use crate::stringify;

/// Primary user-flash region: `USR_*` on new YAMLs, `BANK_*` on legacy ones.
pub(crate) fn primary_flash_regions(chip: &Chip) -> impl Iterator<Item = &MemoryRegion> + Clone {
    chip.memory.iter().filter(|r| {
        r.kind == MemoryRegionKind::Flash
            && (r.name.starts_with("USR_") || r.name.starts_with("BANK_"))
    })
}

pub(crate) fn flash_write_size(r: &MemoryRegion) -> Option<u32> {
    if let Some(s) = &r.settings {
        return Some(s.write_size);
    }
    r.modes.iter().find_map(|m| match m {
        Mode::Fast { page_size, .. } => Some(*page_size),
        _ => None,
    })
}

fn access_attrs(access: Option<&Access>) -> String {
    match access {
        Some(a) => {
            let mut s = String::new();
            if a.read {
                s.push('r');
            }
            if a.write {
                s.push('w');
            }
            if a.execute {
                s.push('x');
            }
            s
        }
        None => "rwx".to_string(),
    }
}

fn format_length(size: u32) -> String {
    if size >= 1024 && size.is_multiple_of(1024) {
        format!("{:>3}K", size / 1024)
    } else {
        format!("{:>4}", size)
    }
}

/// Build a per-option `Vec<MemoryRegion>` by cloning `chip.memory` (which already
/// carries the *default* option's sizes after `resolve.rs`) and overlaying the
/// option's size deltas on top.
fn memory_for_option(chip: &Chip, opt: &MemoryOption) -> Vec<MemoryRegion> {
    let mut memory = chip.memory.clone();
    for (region_name, size) in &opt.region_sizes {
        if let Some(region) = memory.iter_mut().find(|r| &r.name == region_name) {
            region.size = *size;
        }
    }
    memory
}

pub(crate) fn gen_memory_files(
    out_dir: &Path,
    chip: &Chip,
    options: &[MemoryOption],
    default: &str,
) {
    let mem_root = out_dir.join("memory_x");
    if mem_root.exists() {
        fs::remove_dir_all(&mem_root).unwrap();
    }
    for opt in options {
        let memory = memory_for_option(chip, opt);
        let opt_dir = mem_root.join(&opt.name);
        fs::create_dir_all(&opt_dir).unwrap();
        write_memory_x(&opt_dir.join("memory.x"), &memory);
        write_memory_rs(&opt_dir.join("memory.rs"), &memory);
    }
    fs::write(mem_root.join("_default"), default).unwrap();
}

fn write_memory_x(path: &Path, memory: &[MemoryRegion]) {
    let mut memory_x = String::new();
    let usr_1 = memory.iter().find(|r| r.name == "USR_1");

    if let Some(usr) = usr_1 {
        // New schema: real addresses verbatim, CODE alias at 0x00000000 mirroring
        // USR_1 (the WCH boot alias), per-region (rwx) from access flags.
        writeln!(memory_x, "MEMORY").unwrap();
        writeln!(memory_x, "{{").unwrap();
        writeln!(
            memory_x,
            "    {:<5} {:<5} : ORIGIN = 0x00000000, LENGTH = {} /* USR_1 boot alias */",
            "CODE",
            "(rx)",
            format_length(usr.size),
        )
        .unwrap();
        for r in memory {
            let attrs = format!("({})", access_attrs(r.access.as_ref()));
            writeln!(
                memory_x,
                "    {:<5} {:<5} : ORIGIN = 0x{:08X}, LENGTH = {}",
                r.name,
                attrs,
                r.address,
                format_length(r.size),
            )
            .unwrap();
        }
        writeln!(memory_x, "}}").unwrap();
        writeln!(memory_x).unwrap();
        // qingke's link.x still references FLASH; alias until it's updated.
        writeln!(memory_x, r#"REGION_ALIAS("FLASH", CODE);"#).unwrap();
        writeln!(memory_x).unwrap();
        writeln!(memory_x, r#"REGION_ALIAS("REGION_TEXT", CODE);"#).unwrap();
        writeln!(memory_x, r#"REGION_ALIAS("REGION_RODATA", CODE);"#).unwrap();
        writeln!(memory_x, r#"REGION_ALIAS("REGION_DATA", RAM);"#).unwrap();
        writeln!(memory_x, r#"REGION_ALIAS("REGION_BSS", RAM);"#).unwrap();
        writeln!(memory_x, r#"REGION_ALIAS("REGION_HEAP", RAM);"#).unwrap();
        writeln!(memory_x, r#"REGION_ALIAS("REGION_STACK", RAM);"#).unwrap();
    } else {
        // Legacy schema (BANK_*/SRAM/OTP). Preserve the existing flat layout.
        let flash: Vec<&MemoryRegion> = memory
            .iter()
            .filter(|r| {
                r.kind == MemoryRegionKind::Flash
                    && (r.name.starts_with("USR_") || r.name.starts_with("BANK_"))
            })
            .collect();
        let flash_size: u32 = flash.iter().map(|r| r.size).sum();
        let ram = memory
            .iter()
            .find(|r| r.kind == MemoryRegionKind::Ram)
            .unwrap();
        let otp = memory
            .iter()
            .find(|r| r.kind == MemoryRegionKind::Flash && r.name == "OTP");

        write!(memory_x, "MEMORY\n{{\n").unwrap();
        writeln!(
            memory_x,
            "    FLASH : ORIGIN = 0x00000000, LENGTH = {:>4}K /* {} */",
            flash_size / 1024,
            flash
                .iter()
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
    }

    fs::write(path, memory_x).unwrap();
}

fn write_memory_rs(path: &Path, memory: &[MemoryRegion]) {
    let body = format!(
        "use crate::metadata::{{Access, FlashSettings, MemoryRegion, MemoryRegionKind, Mode::*}};

pub static MEMORY: &[MemoryRegion] = {};
",
        stringify(memory)
    );
    fs::write(path, body).unwrap();
}

pub(crate) fn memory_select_cfg_attrs(options: &[MemoryOption], default: &str) -> String {
    let indent = "            ";
    if options.len() == 1 {
        return format!(
            "{}#[path = \"memory_x/{}/memory.rs\"]\n",
            indent, options[0].name
        );
    }
    let non_default: Vec<&str> = options
        .iter()
        .map(|o| o.name.as_str())
        .filter(|n| *n != default)
        .collect();
    let all_not = non_default
        .iter()
        .map(|n| format!("not(feature = \"memory-option-{}\")", n))
        .collect::<Vec<_>>()
        .join(", ");
    let mut s = format!(
        "{}#[cfg_attr(all({}), path = \"memory_x/{}/memory.rs\")]\n",
        indent, all_not, default
    );
    for n in &non_default {
        s.push_str(&format!(
            "{}#[cfg_attr(feature = \"memory-option-{}\", path = \"memory_x/{}/memory.rs\")]\n",
            indent, n, n
        ));
    }
    s
}
