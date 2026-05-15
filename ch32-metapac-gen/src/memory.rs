use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::data::{Access, Chip, MemoryOption, MemoryRegion, MemoryRegionKind, Mode};
use crate::stringify;

pub(crate) fn access_str(access: Option<&Access>) -> String {
    match access {
        Some(a) => format!(
            "{}{}{}",
            if a.read { 'r' } else { '-' },
            if a.write { 'w' } else { '-' },
            if a.execute { 'x' } else { '-' },
        ),
        None => "rwx".to_string(),
    }
}

pub(crate) fn kind_str(kind: &MemoryRegionKind) -> &'static str {
    match kind {
        MemoryRegionKind::Flash => "flash",
        MemoryRegionKind::Ram => "ram",
    }
}

// `USR_*` on new YAMLs, `BANK_*` on legacy ones
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

pub(crate) fn memory_for_option(chip: &Chip, opt: &MemoryOption) -> Vec<MemoryRegion> {
    let mut memory = chip.memory.clone();
    for (region_name, size) in &opt.region_sizes {
        if let Some(region) = memory.iter_mut().find(|r| &r.name == region_name) {
            region.size = *size;
        }
    }
    for (region_name, address) in &opt.region_addresses {
        if let Some(region) = memory.iter_mut().find(|r| &r.name == region_name) {
            region.address = *address;
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
        write_regions(&opt_dir.join("regions"), &memory);
        write_memory_rs(&opt_dir.join("memory.rs"), &memory);
    }
    fs::write(mem_root.join("_default"), default).unwrap();
}

// see `res/memory_x_render.rs` for the line format
fn write_regions(path: &Path, memory: &[MemoryRegion]) {
    let mut s = String::new();
    for r in memory {
        writeln!(
            s,
            "{} 0x{:08x} {} {} {}",
            r.name,
            r.address,
            r.size,
            access_str(r.access.as_ref()),
            kind_str(&r.kind),
        )
        .unwrap();
    }
    fs::write(path, s).unwrap();
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
        .map(|n| format!("not(feature = \"memory-config-{}\")", n))
        .collect::<Vec<_>>()
        .join(", ");
    let mut s = format!(
        "{}#[cfg_attr(all({}), path = \"memory_x/{}/memory.rs\")]\n",
        indent, all_not, default
    );
    for n in &non_default {
        s.push_str(&format!(
            "{}#[cfg_attr(feature = \"memory-config-{}\", path = \"memory_x/{}/memory.rs\")]\n",
            indent, n, n
        ));
    }
    s
}
