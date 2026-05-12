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
