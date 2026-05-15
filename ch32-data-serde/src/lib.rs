use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

mod resolve;
mod size;
pub use size::{deserialize_opt_size_with_suffix, parse_size_with_suffix};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Chip {
    pub name: String,
    pub family: String,
    pub subfamily: String,
    pub product_type: String,
    pub device_id: u32,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub packages: Vec<chip::Package>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_memory: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory: Vec<chip::Memory>,
    #[serde(
        default,
        deserialize_with = "deserialize_memory_options",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub memory_options: BTreeMap<String, BTreeMap<String, u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_memory_option: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_memory_sizes",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub memory_sizes: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_ram_code_config: Option<chip::MemoryRamCodeConfig>,

    pub docs: Vec<chip::Doc>,
    pub cores: Vec<chip::Core>,
}

pub mod chip {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Serialize};

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
        // legacy: superseded by `modes`
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<memory::Settings>,
    }

    pub mod memory {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum Kind {
            Flash,
            Ram,
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(tag = "type", rename_all = "lowercase")]
        pub enum Mode {
            Fast {
                #[serde(deserialize_with = "crate::parse_size_with_suffix")]
                page_size: u32,
                #[serde(deserialize_with = "crate::parse_size_with_suffix")]
                load_size: u32,
            },
            Standard {
                #[serde(deserialize_with = "crate::parse_size_with_suffix")]
                erase_size: u32,
                #[serde(deserialize_with = "crate::parse_size_with_suffix")]
                write_size: u32,
            },
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Access {
            #[serde(default = "crate::default_true")]
            pub read: bool,
            #[serde(default)]
            pub write: bool,
            #[serde(default = "crate::default_true")]
            pub execute: bool,
        }

        // legacy, superseded by `Mode`
        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Settings {
            #[serde(deserialize_with = "crate::parse_size_with_suffix")]
            pub erase_size: u32,
            #[serde(deserialize_with = "crate::parse_size_with_suffix")]
            pub write_size: u32,
            pub erase_value: u8,
        }
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

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Core {
        pub name: String,
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

    pub mod core {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Peripheral {
            pub name: String,
            pub address: u32,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub registers: Option<peripheral::Registers>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub rcc: Option<peripheral::Rcc>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub remap: Option<peripheral::Remap>,
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub pins: Vec<peripheral::Pin>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub interrupts: Option<Vec<peripheral::Interrupt>>, // TODO: This should just be a Vec
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub dma_channels: Vec<peripheral::DmaChannel>,
        }

        pub mod peripheral {
            use serde::{Deserialize, Serialize};

            #[derive(
                Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
            )]
            pub struct Registers {
                pub kind: String,
                pub version: String,
                pub block: String,
            }

            #[derive(
                Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
            )]
            pub struct Rcc {
                pub bus_clock: String,
                pub kernel_clock: rcc::KernelClock,
                pub enable: rcc::Enable,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub reset: Option<rcc::Reset>,
            }

            pub mod rcc {
                use serde::{Deserialize, Serialize};

                #[derive(
                    Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
                )]
                pub struct Field {
                    pub register: String,
                    pub field: String,
                }

                #[derive(
                    Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
                )]
                #[serde(untagged)]
                pub enum KernelClock {
                    Clock(String),
                    Mux(Field),
                }

                #[derive(
                    Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
                )]
                pub struct Enable {
                    pub register: String,
                    pub field: String,
                }

                #[derive(
                    Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
                )]
                pub struct Reset {
                    pub register: String,
                    pub field: String,
                }
            }

            #[derive(
                Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
            )]
            pub struct Remap {
                pub register: String,
                pub field: String,
            }

            #[derive(
                Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
            )]
            pub struct Pin {
                pub pin: pin::Pin,
                pub signal: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub remap: Option<u8>,
            }

            pub mod pin {
                use serde::{Deserialize, Serialize};

                #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
                pub struct Pin {
                    pub port: char,
                    pub num: u8,
                }

                impl Pin {
                    pub fn parse(pin: &str) -> Option<Self> {
                        let mut chars = pin.chars();
                        let p = chars.next()?;
                        if p != 'P' {
                            return None;
                        }
                        let port = chars.next()?;
                        let num = chars.as_str().parse().ok()?;

                        Some(Self { port, num })
                    }
                }

                impl std::fmt::Display for Pin {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(f, "P{}{}", self.port, self.num)
                    }
                }

                impl Serialize for Pin {
                    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                    where
                        S: serde::Serializer,
                    {
                        serializer.serialize_str(&format!("{self}"))
                    }
                }

                struct PinVisitor;

                impl<'de> serde::de::Visitor<'de> for PinVisitor {
                    type Value = Pin;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("pin")
                    }

                    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        Ok(Pin::parse(v).unwrap())
                    }
                }

                impl<'de> Deserialize<'de> for Pin {
                    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                    where
                        D: serde::Deserializer<'de>,
                    {
                        deserializer.deserialize_str(PinVisitor)
                    }
                }
            }

            #[derive(
                Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
            )]
            pub struct Interrupt {
                pub signal: String,
                pub interrupt: String,
            }

            #[derive(
                Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize,
            )]
            pub struct DmaChannel {
                pub signal: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub dma: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub channel: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub dmamux: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub request: Option<u8>,
            }
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct Interrupt {
            pub name: String,
            pub number: u8,
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct DmaChannels {
            pub name: String,
            pub dma: String,
            pub channel: u8,
        }
    }
}

fn default_true() -> bool {
    true
}

fn deserialize_memory_options<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, BTreeMap<String, u32>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct WrappedSize(#[serde(deserialize_with = "parse_size_with_suffix")] u32);

    let raw: BTreeMap<String, BTreeMap<String, WrappedSize>> = BTreeMap::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|(k, inner)| {
            (
                k,
                inner
                    .into_iter()
                    .map(|(rk, WrappedSize(v))| (rk, v))
                    .collect(),
            )
        })
        .collect())
}

fn deserialize_memory_sizes<'de, D>(deserializer: D) -> Result<BTreeMap<String, u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct WrappedSize(#[serde(deserialize_with = "parse_size_with_suffix")] u32);

    let raw: BTreeMap<String, WrappedSize> = BTreeMap::deserialize(deserializer)?;
    Ok(raw.into_iter().map(|(k, WrappedSize(v))| (k, v)).collect())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::path::Path;
    use std::{fs, str};

    use super::*;

    fn normalize_line(line: &str) -> Cow<'_, str> {
        // The python script saves with 4 spaces instead of 2
        let line = line.trim_start();

        // The python script escapes unicode
        let mut line = Cow::Borrowed(line);
        for symbol in [("\\u00ae", "\u{00ae}"), ("\\u2122", "\u{2122}")] {
            if line.contains(symbol.0) {
                line = Cow::Owned(line.replace(symbol.0, symbol.1));
            }
        }

        line
    }

    fn normalize(file: &[u8]) -> impl Iterator<Item = Cow<'_, str>> + '_ {
        str::from_utf8(file).unwrap().lines().map(normalize_line)
    }

    fn check_file(path: impl AsRef<Path>) {
        println!("Checking {:?}", path.as_ref());
        let original = fs::read(path).unwrap();
        let parsed: Chip = serde_yaml::from_slice(&original).unwrap();
        let reencoded = serde_yaml::to_string(&parsed).unwrap();
        //   println!("{:?}", parsed);
        // itertools::assert_equal(normalize(&original), normalize(&reencoded))
    }

    const CHIPS_DIR: &str = "data/chips/";

    #[test]
    fn test_one() {
        let path = Path::new(CHIPS_DIR).join("CH32X035G8U6.yaml");
        check_file(path);
    }

    #[test]
    fn test_all() {
        use rayon::prelude::*;

        Path::new(CHIPS_DIR)
            .read_dir()
            .unwrap()
            .par_bridge()
            .for_each(|chip| {
                check_file(chip.unwrap().path());
            });
    }
}
