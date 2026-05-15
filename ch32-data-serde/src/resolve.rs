use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{chip, chip::memory, Chip};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
pub(crate) struct FamilyMemory {
    #[serde(default)]
    pub include_memory: Option<String>,
    #[serde(default)]
    pub memory: Vec<chip::Memory>,
}

fn load_recursive<F, E>(
    path: &Path,
    load_family: &mut F,
    visited: &mut Vec<PathBuf>,
) -> Result<Vec<chip::Memory>, String>
where
    F: FnMut(&Path) -> Result<String, E>,
    E: fmt::Display,
{
    let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if visited.contains(&key) {
        let chain = visited
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(format!(
            "cycle in include_memory chain: {chain} -> {}",
            path.display()
        ));
    }
    visited.push(key);

    let content = load_family(path)
        .map_err(|e| format!("read memory include {:?}: {e}", path.display()))?;
    let family: FamilyMemory = serde_yaml::from_str(&content)
        .map_err(|e| format!("parse memory include {:?}: {e}", path.display()))?;

    let mut combined = if let Some(parent) = family.include_memory {
        let parent_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let parent_path = parent_dir.join(&parent);
        load_recursive(&parent_path, load_family, visited)?
    } else {
        Vec::new()
    };

    for region in family.memory {
        if combined.iter().any(|r| r.name == region.name) {
            return Err(format!(
                "memory region {:?} from {:?} collides with a region from a parent include \
                 (include_memory appends; each region must be declared exactly once in a chain)",
                region.name,
                path.display()
            ));
        }
        combined.push(region);
    }

    visited.pop();
    Ok(combined)
}

impl Chip {
    pub fn resolve_memory<F, E>(&mut self, chip_dir: &Path, mut load_family: F) -> Result<(), String>
    where
        F: FnMut(&Path) -> Result<String, E>,
        E: fmt::Display,
    {
        let include = self.include_memory.take();
        let uses_new_schema = include.is_some()
            || !self.memory_sizes.is_empty()
            || self.memory_ram_code_config.is_some();

        if !self.memory_sizes.is_empty() && self.memory_ram_code_config.is_some() {
            return Err(format!(
                "chip {:?}: memory_sizes and memory_ram_code_config are mutually exclusive \
                 (memory_ram_code_config already determines all region sizes)",
                self.name
            ));
        }

        if let Some(inc_path) = include {
            let absolute = chip_dir.join(&inc_path);
            let mut visited = Vec::new();
            let regions = load_recursive(&absolute, &mut load_family, &mut visited)?;
            for region in regions {
                if self.memory.iter().any(|r| r.name == region.name) {
                    return Err(format!(
                        "memory region {:?} on chip {:?} collides with a region from {inc_path:?} \
                         (include_memory appends; sizes go through memory_sizes or memory_ram_code_config)",
                        region.name, self.name
                    ));
                }
                self.memory.push(region);
            }
        }

        for (region_name, size) in &self.memory_sizes {
            let region = self
                .memory
                .iter_mut()
                .find(|r| &r.name == region_name)
                .ok_or_else(|| {
                    format!(
                        "memory_sizes references unknown region {region_name:?} for chip {:?}",
                        self.name
                    )
                })?;
            region.size = Some(*size);
        }

        if let Some(config) = &self.memory_ram_code_config {
            let default_opt = config
                .configs
                .iter()
                .find(|c| c.name == config.default)
                .ok_or_else(|| {
                    format!(
                        "memory_ram_code_config.default {:?} not in configs for chip {:?}",
                        config.default, self.name
                    )
                })?
                .clone();

            let (usr1_address, usr1_modes, usr1_access, usr1_cores) = {
                let usr1 = self
                    .memory
                    .iter_mut()
                    .find(|r| r.name == "USR_1")
                    .ok_or_else(|| {
                        format!(
                            "memory_ram_code_config requires a USR_1 region for chip {:?}",
                            self.name
                        )
                    })?;
                usr1.size = Some(default_opt.code);
                let address = usr1.address.ok_or_else(|| {
                    format!(
                        "USR_1 region for chip {:?} has no address (memory_ram_code_config needs it to place USR_2)",
                        self.name
                    )
                })?;
                (address, usr1.modes.clone(), usr1.access.clone(), usr1.cores.clone())
            };

            let ram = self
                .memory
                .iter_mut()
                .find(|r| r.name == "RAM")
                .ok_or_else(|| {
                    format!(
                        "memory_ram_code_config requires a RAM region for chip {:?}",
                        self.name
                    )
                })?;
            ram.size = Some(default_opt.ram);

            if config.total_flash < default_opt.code {
                return Err(format!(
                    "memory_ram_code_config: total_flash ({}) < default config code ({}) for chip {:?}",
                    config.total_flash, default_opt.code, self.name
                ));
            }
            let usr2_size = config.total_flash - default_opt.code;
            if !self.memory.iter().any(|r| r.name == "USR_2") {
                self.memory.push(chip::Memory {
                    name: "USR_2".to_string(),
                    kind: Some(memory::Kind::Flash),
                    address: Some(usr1_address + default_opt.code),
                    size: Some(usr2_size),
                    modes: usr1_modes,
                    access: usr1_access,
                    cores: usr1_cores,
                    structs: Vec::new(),
                });
            }
        }

        for r in &self.memory {
            if r.kind.is_none() || r.address.is_none() || r.size.is_none() {
                return Err(format!(
                    "memory region {:?} on chip {:?} is missing kind/address/size after resolution",
                    r.name, self.name
                ));
            }
        }

        if uses_new_schema && !self.memory.iter().any(|r| r.name == "USR_1") {
            return Err(format!(
                "chip {:?} has no USR_1 memory region; USR_1 is the primary user flash and must be defined \
                 (additional banks use USR_2, USR_3, ...; system flash banks use SYS_1, SYS_2, ...)",
                self.name
            ));
        }

        Ok(())
    }
}
