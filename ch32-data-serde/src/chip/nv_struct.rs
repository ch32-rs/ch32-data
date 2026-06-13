use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NvStruct {
    pub name: String,
    #[serde(deserialize_with = "crate::parse_size_with_suffix")]
    pub offset: u32,
    pub kind: String,
    pub version: String,
    pub block: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub defaults: BTreeMap<String, u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versioned_with_defaults() {
        let yaml = r#"
name: OB
offset: 0
kind: ob
version: v0
block: OB
defaults:
  USER: 0xFE
"#;
        let parsed: NvStruct = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.name, "OB");
        assert_eq!(parsed.offset, 0);
        assert_eq!(parsed.kind, "ob");
        assert_eq!(parsed.version, "v0");
        assert_eq!(parsed.block, "OB");
        assert_eq!(parsed.defaults.get("USER"), Some(&0xFE));
    }

    #[test]
    fn parses_common_version_without_defaults() {
        let yaml = r#"
name: ESIG
offset: 0x20
kind: esig
version: common
block: ESIG
"#;
        let parsed: NvStruct = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(parsed.name, "ESIG");
        assert_eq!(parsed.offset, 0x20);
        assert_eq!(parsed.kind, "esig");
        assert_eq!(parsed.version, "common");
        assert_eq!(parsed.block, "ESIG");
        assert!(parsed.defaults.is_empty());
    }

    #[test]
    fn round_trips() {
        let original = NvStruct {
            name: "OB".into(),
            offset: 0,
            kind: "ob".into(),
            version: "v0".into(),
            block: "OB".into(),
            defaults: [("USER".into(), 0xFE_u32)].into_iter().collect(),
        };
        let yaml = serde_yaml::to_string(&original).unwrap();
        let reparsed: NvStruct = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(original, reparsed);
    }
}
