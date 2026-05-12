use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::chip::build_memory_options;
use crate::data::{Chip, MemoryRegion};
use crate::memory::{access_str, kind_str, memory_for_option};
use crate::memory_x_render::{Region, split_name};

pub struct ChipOption {
    pub name: String,
    pub regions: Vec<Region>,
}

pub struct ChipDump {
    pub name: String,
    pub default_option: String,
    pub options: Vec<ChipOption>,
}

pub fn list_chips(data_dir: &Path) -> Vec<String> {
    let chips_dir = data_dir.join("chips");
    let mut names: Vec<String> = fs::read_dir(&chips_dir)
        .unwrap_or_else(|e| panic!("read {}: {}", chips_dir.display(), e))
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let n = e.file_name().to_str()?.to_string();
            n.strip_suffix(".json").map(|s| s.to_string())
        })
        .collect();
    names.sort();
    names
}

pub fn load_chip(data_dir: &Path, chip_name: &str) -> ChipDump {
    let path = data_dir.join("chips").join(format!("{}.json", chip_name));
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let chip: Chip = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));

    let options = build_memory_options(&chip);
    let default_option = chip
        .default_memory_option
        .clone()
        .unwrap_or_else(|| "default".to_string());

    let options = options
        .iter()
        .map(|opt| ChipOption {
            name: opt.name.clone(),
            regions: memory_for_option(&chip, opt)
                .iter()
                .map(to_render_region)
                .collect(),
        })
        .collect();

    ChipDump {
        name: chip.name,
        default_option,
        options,
    }
}

pub fn split_prefixes_from_names<'a, I: IntoIterator<Item = &'a str>>(names: I) -> Vec<String> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for n in names {
        if let Some((prefix, _)) = split_name(n) {
            *counts.entry(prefix.to_ascii_lowercase()).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(_, n)| *n >= 2)
        .map(|(p, _)| p)
        .collect()
}

fn to_render_region(r: &MemoryRegion) -> Region {
    Region {
        name: r.name.clone(),
        address: r.address,
        size: r.size,
        access: access_str(r.access.as_ref()),
        kind: kind_str(&r.kind).to_string(),
    }
}
