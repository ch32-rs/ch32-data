use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod chip;
mod resolve;
mod size;
pub use size::{deserialize_opt_size_with_suffix, parse_size_with_suffix};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Chip {
    pub name: String,
    pub family: String,
    pub subfamily: String,
    pub product_type: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    pub packages: Vec<chip::Package>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_memory: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory: Vec<chip::Memory>,
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

fn default_true() -> bool {
    true
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
