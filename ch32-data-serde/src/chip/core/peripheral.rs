use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Registers {
    pub kind: String,
    pub version: String,
    pub block: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Rcc {
    pub bus_clock: String,
    pub kernel_clock: rcc::KernelClock,
    pub enable: rcc::Enable,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset: Option<rcc::Reset>,
}

pub mod rcc {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Field {
        pub register: String,
        pub field: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum KernelClock {
        Clock(String),
        Mux(Field),
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Enable {
        pub register: String,
        pub field: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
    pub struct Reset {
        pub register: String,
        pub field: String,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Remap {
    pub register: String,
    pub field: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Interrupt {
    pub signal: String,
    pub interrupt: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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
