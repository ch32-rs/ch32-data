//! Runtime descriptor overlay for non-volatile flash structures (Option Bytes, ESIG, ...).
//!
//! Discovers `NvStruct` metadata attached to the active chip's memory regions, then exposes
//! a path-based API (`descriptor.entry[.field]`) for reading, writing, and validating buffers
//! laid out per those descriptors.

use crate::metadata::{
    ir::{self, Access, Enum},
    MemoryRegion, NvStruct, METADATA,
};

#[derive(Debug, Clone, Copy)]
pub struct Descriptor {
    region: &'static MemoryRegion,
    nv: &'static NvStruct,
}

#[derive(Debug, Clone)]
pub struct Info {
    pub description: Option<&'static str>,
    pub byte_offset: u32,
    pub bit_offset: u32,
    pub bit_size: u32,
    pub access: Access,
    pub enumm: Option<&'static Enum>,
    pub default: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub enum Value<'a> {
    Variant(&'static str),
    Literal(u64),
    Bytes(&'a [u8]),
}

#[derive(Debug, Clone, Copy)]
pub enum EncodeInput<'a> {
    Variant(&'a str),
    Literal(u64),
    Bytes(&'a [u8]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    ReadOnly,
    NoSuchEntry,
    NoSuchField,
    NoSuchVariant,
    OutOfRange,
    BufferTooShort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    ComplementMismatch { entry: &'static str },
    BufferTooShort,
}

impl Descriptor {
    pub fn iter() -> impl Iterator<Item = Descriptor> {
        METADATA.memory.iter().flat_map(|region| {
            region
                .structs
                .iter()
                .map(move |nv| Descriptor { region, nv })
        })
    }

    pub fn find(name: &str) -> Option<Descriptor> {
        Self::iter().find(|d| d.nv.kind.eq_ignore_ascii_case(name))
    }

    pub fn name(&self) -> &'static str {
        self.nv.name
    }

    pub fn kind(&self) -> &'static str {
        self.nv.kind
    }

    fn block(&self) -> Option<&'static ir::Block> {
        self.nv.ir.blocks.iter().find(|b| b.name == self.nv.block)
    }

    fn item(&self, entry: &str) -> Option<&'static ir::BlockItem> {
        self.block()?
            .items
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case(entry))
    }

    fn register_of(item: &ir::BlockItem) -> Option<&ir::Register> {
        match &item.inner {
            ir::BlockItemInner::Register(r) => Some(r),
            _ => None,
        }
    }

    fn fieldset_of(&self, item: &ir::BlockItem) -> Option<&'static ir::FieldSet> {
        let reg = Self::register_of(item)?;
        let name = reg.fieldset?;
        self.nv.ir.fieldsets.iter().find(|fs| fs.name == name)
    }

    fn field_in(fs: &'static ir::FieldSet, name: &str) -> Option<&'static ir::Field> {
        fs.fields.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }

    fn enumm_named(&self, name: &str) -> Option<&'static ir::Enum> {
        self.nv.ir.enums.iter().find(|e| e.name == name)
    }

    fn default_for(&self, entry: &str) -> Option<u64> {
        self.nv
            .defaults
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(entry))
            .map(|(_, v)| *v as u64)
    }

    pub fn entries(&self) -> impl Iterator<Item = &'static str> {
        self.block()
            .into_iter()
            .flat_map(|b| b.items.iter().map(|i| i.name))
    }

    pub fn fields(&self, entry: &str) -> Option<impl Iterator<Item = &'static str>> {
        let item = self.item(entry)?;
        let fs = self.fieldset_of(item)?;
        Some(fs.fields.iter().map(|f| f.name))
    }

    pub fn describe(&self, path: &str) -> Option<Info> {
        let (entry_name, field_name) = match path.split_once('.') {
            Some((e, f)) => (e, Some(f)),
            None => (path, None),
        };
        let item = self.item(entry_name)?;
        let reg = Self::register_of(item)?;

        match field_name {
            None => Some(Info {
                description: item.description,
                byte_offset: item.byte_offset,
                bit_offset: 0,
                bit_size: reg.bit_size,
                access: reg.access.clone(),
                enumm: None,
                default: self.default_for(entry_name),
            }),
            Some(fname) => {
                let fs = self.fieldset_of(item)?;
                let field = Self::field_in(fs, fname)?;
                let bit_offset = match &field.bit_offset {
                    ir::BitOffset::Regular(r) => r.offset,
                    ir::BitOffset::Cursed(_) => return None,
                };
                let enumm = field.enumm.and_then(|n| self.enumm_named(n));
                Some(Info {
                    description: field.description,
                    byte_offset: item.byte_offset,
                    bit_offset,
                    bit_size: field.bit_size,
                    access: reg.access.clone(),
                    enumm,
                    default: None,
                })
            }
        }
    }

    pub fn decode<'a>(&self, buf: &'a [u8], path: &str) -> Option<Value<'a>> {
        let info = self.describe(path)?;
        let start = info.byte_offset as usize;

        if info.bit_size > 64 {
            // Whole-entry blob (bit_offset always 0 for entries).
            let end = start + ((info.bit_size + 7) / 8) as usize;
            if end > buf.len() {
                return None;
            }
            return Some(Value::Bytes(&buf[start..end]));
        }

        let bit_end = info.bit_offset + info.bit_size;
        let byte_end = start + ((bit_end + 7) / 8) as usize;
        if byte_end > buf.len() {
            return None;
        }

        let mut raw: u64 = 0;
        for i in 0..(byte_end - start) {
            raw |= (buf[start + i] as u64) << (8 * i);
        }
        let mask = if info.bit_size == 64 {
            u64::MAX
        } else {
            (1u64 << info.bit_size) - 1
        };
        let value = (raw >> info.bit_offset) & mask;

        if let Some(enumm) = info.enumm {
            if let Some(v) = enumm.variants.iter().find(|v| v.value == value) {
                return Some(Value::Variant(v.name));
            }
        }
        Some(Value::Literal(value))
    }

    pub fn encode(
        &self,
        _buf: &mut [u8],
        _path: &str,
        _input: EncodeInput,
    ) -> Result<(), EncodeError> {
        unimplemented!()
    }

    pub fn validate(&self, _buf: &[u8]) -> Result<(), ValidationError> {
        unimplemented!()
    }

    pub fn reset_to_defaults(&self, _buf: &mut [u8]) {
        unimplemented!()
    }
}

pub fn decode<'a>(path: &str, buf: &'a [u8]) -> Option<Value<'a>> {
    let (descriptor, rest) = path.split_once('.')?;
    Descriptor::find(descriptor)?.decode(buf, rest)
}

pub fn encode(_path: &str, _buf: &mut [u8], _input: EncodeInput) -> Result<(), EncodeError> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(it: impl Iterator<Item = &'static str>, needle: &str) -> bool {
        it.into_iter().any(|s| s == needle)
    }

    #[test]
    fn discovery_finds_ob_and_esig() {
        assert!(has(Descriptor::iter().map(|d| d.kind()), "ob"));
        assert!(has(Descriptor::iter().map(|d| d.kind()), "esig"));
        assert!(Descriptor::find("OB").is_some());
        assert!(Descriptor::find("esig").is_some());
        assert!(Descriptor::find("nope").is_none());
    }

    #[test]
    fn ob_entries_and_fields() {
        let ob = Descriptor::find("ob").unwrap();
        assert!(has(ob.entries(), "RDPR"));
        assert!(has(ob.entries(), "USER"));
        assert!(has(ob.entries(), "NRDPR"));

        let user_fields = ob.fields("user").unwrap();
        assert!(has(user_fields, "IWDGSW"));
        let user_fields = ob.fields("user").unwrap();
        assert!(has(user_fields, "START_MODE"));

        // Entries without a fieldset (e.g. NRDPR complement) have no fields.
        assert!(ob.fields("NRDPR").is_none());
    }

    #[test]
    fn describe_entry_and_field() {
        let ob = Descriptor::find("ob").unwrap();

        let user = ob.describe("user").unwrap();
        assert_eq!(user.byte_offset, 2);
        assert_eq!(user.bit_offset, 0);
        assert_eq!(user.bit_size, 8);
        assert!(user.enumm.is_none());
        assert_eq!(user.default, Some(247));

        let iwdg = ob.describe("user.iwdgsw").unwrap();
        assert_eq!(iwdg.byte_offset, 2);
        assert_eq!(iwdg.bit_offset, 0);
        assert_eq!(iwdg.bit_size, 1);
        assert_eq!(iwdg.enumm.unwrap().name, "IwdgMode");
        assert!(iwdg.default.is_none());

        // Read-only complement.
        let nrdpr = ob.describe("nrdpr").unwrap();
        assert!(matches!(nrdpr.access, Access::Read));
    }

    #[test]
    fn decode_literal_and_variant_from_ob_defaults() {
        // OB default layout (from NvStruct.defaults), byte order RDPR=0xa5 NRDPR=0x5a USER=0xf7 NUSER=0x08 ...
        let buf = [
            0xa5, 0x5a, 0xf7, 0x08, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00,
        ];

        // Whole USER byte literal.
        assert!(matches!(
            decode("ob.user", &buf),
            Some(Value::Literal(0xf7))
        ));

        // IWDGSW is bit 0 of USER → 1 → SOFTWARE.
        assert!(matches!(
            decode("ob.user.iwdgsw", &buf),
            Some(Value::Variant("SOFTWARE"))
        ));

        // RDPR=0xa5 → UNPROTECTED variant.
        assert!(matches!(
            decode("ob.rdpr.rdpr", &buf),
            Some(Value::Variant("UNPROTECTED"))
        ));
    }

    #[test]
    fn decode_esig_uniid() {
        let esig = Descriptor::find("esig").unwrap();
        let mut buf = [0u8; 32];
        // UNIID1 at byte_offset 8, 32 bits LE.
        buf[8..12].copy_from_slice(&0xdead_beef_u32.to_le_bytes());
        let v = esig.decode(&buf, "uniid1").unwrap();
        assert!(matches!(v, Value::Literal(0xdead_beef)));
    }

    #[test]
    fn decode_out_of_range_returns_none() {
        assert!(decode("ob.nope", &[0; 12]).is_none());
        assert!(decode("ob.user.nope", &[0; 12]).is_none());
        assert!(decode("nope.user", &[0; 12]).is_none());
        // Short buffer.
        assert!(decode("ob.user", &[]).is_none());
    }
}
