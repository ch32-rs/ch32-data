use std::fmt;

// accepts an integer or a string with a K/KB/KiB suffix
pub fn parse_size_with_suffix<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct SizeVisitor;

    impl<'de> serde::de::Visitor<'de> for SizeVisitor {
        type Value = u32;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a byte size as an integer or a string like \"16K\" or \"2048\"")
        }

        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<u32, E> {
            u32::try_from(v).map_err(E::custom)
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<u32, E> {
            u32::try_from(v).map_err(E::custom)
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<u32, E> {
            parse_size_str(v).map_err(E::custom)
        }
        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<u32, E> {
            parse_size_str(&v).map_err(E::custom)
        }
    }

    deserializer.deserialize_any(SizeVisitor)
}

pub fn deserialize_opt_size_with_suffix<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct OptSizeVisitor;

    impl<'de> serde::de::Visitor<'de> for OptSizeVisitor {
        type Value = Option<u32>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an optional byte size (integer or string)")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_unit<E>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
        fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            parse_size_with_suffix(d).map(Some)
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
            u32::try_from(v).map(Some).map_err(E::custom)
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
            u32::try_from(v).map(Some).map_err(E::custom)
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
            parse_size_str(v).map(Some).map_err(E::custom)
        }
        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
            parse_size_str(&v).map(Some).map_err(E::custom)
        }
    }

    deserializer.deserialize_any(OptSizeVisitor)
}

fn parse_size_str(s: &str) -> Result<u32, String> {
    let s = s.trim();
    let (num, mul) = if let Some(n) = s.strip_suffix("KiB") {
        (n, 1024)
    } else if let Some(n) = s.strip_suffix("KB") {
        (n, 1024)
    } else if let Some(n) = s.strip_suffix("K") {
        (n, 1024)
    } else {
        (s, 1)
    };
    num.trim()
        .parse::<u32>()
        .map_err(|e| format!("bad size {s:?}: {e}"))
        .map(|v| v * mul)
}
