use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod core;
pub mod memory;
pub mod nv_struct;

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub package: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Memory {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<memory::Kind>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_opt_size_with_suffix",
        skip_serializing_if = "Option::is_none"
    )]
    pub address: Option<u32>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_opt_size_with_suffix",
        skip_serializing_if = "Option::is_none"
    )]
    pub size: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<memory::Mode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<memory::Access>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structs: Vec<nv_struct::NvStruct>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemoryRamCodeConfig {
    #[serde(deserialize_with = "crate::parse_size_with_suffix")]
    pub total_flash: u32,
    pub default: String,
    pub configs: Vec<MemoryRamCodeOption>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemoryRamCodeOption {
    pub name: String,
    #[serde(deserialize_with = "crate::parse_size_with_suffix")]
    pub code: u32,
    #[serde(deserialize_with = "crate::parse_size_with_suffix")]
    pub ram: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Doc {
    pub r#type: String,
    pub title: String,
    pub name: String,
    pub url: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    Riscv,
    Arm,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Core {
    pub name: String,
    pub arch: Arch,
    #[serde(default)]
    pub peripherals: Vec<core::Peripheral>,
    #[serde(default)]
    pub interrupts: Vec<core::Interrupt>,
    #[serde(default)]
    pub dma_channels: Vec<core::DmaChannels>,

    // include fields, for common peripherals
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_interrupts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_dma_channels: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_peripherals: Option<Vec<String>>,
}
