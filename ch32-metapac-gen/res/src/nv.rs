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
        buf: &mut [u8],
        path: &str,
        input: EncodeInput,
    ) -> Result<(), EncodeError> {
        let (entry_name, field_name) = match path.split_once('.') {
            Some((e, f)) => (e, Some(f)),
            None => (path, None),
        };
        let item = self.item(entry_name).ok_or(EncodeError::NoSuchEntry)?;
        let reg = Self::register_of(item).ok_or(EncodeError::NoSuchEntry)?;
        if matches!(reg.access, Access::Read) {
            return Err(EncodeError::ReadOnly);
        }

        let entry_start = item.byte_offset as usize;
        let entry_size = ((reg.bit_size + 7) / 8) as usize;
        if entry_start + entry_size > buf.len() {
            return Err(EncodeError::BufferTooShort);
        }

        match field_name {
            None => write_entry(&mut buf[entry_start..entry_start + entry_size], reg.bit_size, input)?,
            Some(fname) => {
                let fs = self.fieldset_of(item).ok_or(EncodeError::NoSuchField)?;
                let field = Self::field_in(fs, fname).ok_or(EncodeError::NoSuchField)?;
                let bit_offset = match &field.bit_offset {
                    ir::BitOffset::Regular(r) => r.offset,
                    ir::BitOffset::Cursed(_) => return Err(EncodeError::NoSuchField),
                };
                let value = resolve_field_value(field, self, input)?;
                write_field_bits(
                    &mut buf[entry_start..entry_start + entry_size],
                    bit_offset,
                    field.bit_size,
                    value,
                );
            }
        }

        self.apply_complement(buf, item)
    }

    fn apply_complement(&self, buf: &mut [u8], item: &ir::BlockItem) -> Result<(), EncodeError> {
        let block = match self.block() {
            Some(b) => b,
            None => return Ok(()),
        };
        for sibling in block.items {
            if !is_n_complement(sibling.name, item.name) {
                continue;
            }
            let sib_reg = match Self::register_of(sibling) {
                Some(r) => r,
                None => continue,
            };
            let src_size = match Self::register_of(item) {
                Some(r) => ((r.bit_size + 7) / 8) as usize,
                None => continue,
            };
            let dst_size = ((sib_reg.bit_size + 7) / 8) as usize;
            if src_size != dst_size {
                continue;
            }
            let src = item.byte_offset as usize;
            let dst = sibling.byte_offset as usize;
            if dst + dst_size > buf.len() {
                return Err(EncodeError::BufferTooShort);
            }
            for i in 0..src_size {
                buf[dst + i] = !buf[src + i];
            }
            return Ok(());
        }
        Ok(())
    }

    pub fn validate(&self, buf: &[u8]) -> Result<(), ValidationError> {
        let block = match self.block() {
            Some(b) => b,
            None => return Ok(()),
        };
        for item in block.items {
            let reg = match Self::register_of(item) {
                Some(r) => r,
                None => continue,
            };
            let entry_size = ((reg.bit_size + 7) / 8) as usize;
            let sibling = block
                .items
                .iter()
                .find(|s| is_n_complement(s.name, item.name));
            let Some(sibling) = sibling else { continue };
            let src = item.byte_offset as usize;
            let dst = sibling.byte_offset as usize;
            if src + entry_size > buf.len() || dst + entry_size > buf.len() {
                return Err(ValidationError::BufferTooShort);
            }
            for i in 0..entry_size {
                if buf[dst + i] != !buf[src + i] {
                    return Err(ValidationError::ComplementMismatch { entry: item.name });
                }
            }
        }
        Ok(())
    }

    /// Apply every default value from `NvStruct.defaults` then re-sync every
    /// `N`-complement so the buffer is internally consistent. Read-only entries
    /// in `defaults` (e.g. the `N`-complement bytes the YAML carries through)
    /// are skipped — their value is recomputed from their source entry. No-op
    /// on descriptors with an empty `defaults` list (e.g. ESIG).
    pub fn reset(&self, buf: &mut [u8]) -> Result<(), EncodeError> {
        if self.nv.defaults.is_empty() {
            return Ok(());
        }
        for (entry_name, value) in self.nv.defaults {
            let Some(item) = self.item(entry_name) else { continue };
            let Some(reg) = Self::register_of(item) else { continue };
            if matches!(reg.access, Access::Read) {
                continue;
            }
            self.encode(buf, entry_name, EncodeInput::Literal(*value as u64))?;
        }
        let Some(block) = self.block() else { return Ok(()) };
        for item in block.items {
            self.apply_complement(buf, item)?;
        }
        Ok(())
    }
}

fn is_n_complement(sibling: &str, source: &str) -> bool {
    let s = sibling.as_bytes();
    let src = source.as_bytes();
    s.len() == src.len() + 1
        && s[0].eq_ignore_ascii_case(&b'N')
        && s[1..].eq_ignore_ascii_case(src)
}

fn bit_mask(bit_size: u32) -> u64 {
    if bit_size >= 64 {
        u64::MAX
    } else {
        (1u64 << bit_size) - 1
    }
}

fn write_entry(dst: &mut [u8], bit_size: u32, input: EncodeInput) -> Result<(), EncodeError> {
    match input {
        EncodeInput::Bytes(bytes) => {
            if bytes.len() != dst.len() {
                return Err(EncodeError::OutOfRange);
            }
            dst.copy_from_slice(bytes);
            Ok(())
        }
        EncodeInput::Literal(val) => {
            if bit_size < 64 && val > bit_mask(bit_size) {
                return Err(EncodeError::OutOfRange);
            }
            for i in 0..dst.len() {
                dst[i] = ((val >> (8 * i)) & 0xff) as u8;
            }
            Ok(())
        }
        // Entries themselves have no enum; variants only resolve at field level.
        EncodeInput::Variant(_) => Err(EncodeError::NoSuchVariant),
    }
}

fn resolve_field_value(
    field: &ir::Field,
    desc: &Descriptor,
    input: EncodeInput,
) -> Result<u64, EncodeError> {
    let max = bit_mask(field.bit_size);
    match input {
        EncodeInput::Variant(name) => {
            let enumm = field
                .enumm
                .and_then(|n| desc.enumm_named(n))
                .ok_or(EncodeError::NoSuchVariant)?;
            enumm
                .variants
                .iter()
                .find(|v| v.name.eq_ignore_ascii_case(name))
                .map(|v| v.value)
                .ok_or(EncodeError::NoSuchVariant)
        }
        EncodeInput::Literal(v) => {
            if v > max {
                Err(EncodeError::OutOfRange)
            } else {
                Ok(v)
            }
        }
        // Raw byte writes only make sense at entry level.
        EncodeInput::Bytes(_) => Err(EncodeError::OutOfRange),
    }
}

fn write_field_bits(dst: &mut [u8], bit_offset: u32, bit_size: u32, value: u64) {
    let bit_end = bit_offset + bit_size;
    let bytes_touched = ((bit_end + 7) / 8) as usize;
    let mut raw: u64 = 0;
    for i in 0..bytes_touched {
        raw |= (dst[i] as u64) << (8 * i);
    }
    let mask = bit_mask(bit_size) << bit_offset;
    raw = (raw & !mask) | ((value << bit_offset) & mask);
    for i in 0..bytes_touched {
        dst[i] = ((raw >> (8 * i)) & 0xff) as u8;
    }
}

pub fn decode<'a>(path: &str, buf: &'a [u8]) -> Option<Value<'a>> {
    let (descriptor, rest) = path.split_once('.')?;
    Descriptor::find(descriptor)?.decode(buf, rest)
}

pub fn encode(path: &str, buf: &mut [u8], input: EncodeInput) -> Result<(), EncodeError> {
    let (descriptor, rest) = path.split_once('.').ok_or(EncodeError::NoSuchEntry)?;
    Descriptor::find(descriptor)
        .ok_or(EncodeError::NoSuchEntry)?
        .encode(buf, rest, input)
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use crate::metadata::ir;
    use alloc::format;
    use alloc::vec::Vec;

    const BUF: usize = 256;

    fn block_of(d: &Descriptor) -> &'static ir::Block {
        d.block().expect("descriptor has block")
    }

    fn writable(item: &'static ir::BlockItem) -> Option<&'static ir::Register> {
        match Descriptor::register_of(item) {
            Some(r) if !matches!(r.access, Access::Read) => Some(r),
            _ => None,
        }
    }

    fn decoded_to_u64(d: &Descriptor, path: &str, v: Value) -> u64 {
        match v {
            Value::Literal(x) => x,
            Value::Variant(name) => {
                let info = d.describe(path).unwrap();
                let enumm = info.enumm.expect("Value::Variant requires an enum");
                enumm
                    .variants
                    .iter()
                    .find(|v| v.name == name)
                    .unwrap()
                    .value
            }
            Value::Bytes(_) => panic!("unexpected Value::Bytes for {}", path),
        }
    }

    /// Pre-fill `buf` so every writable entry has been encoded once; this leaves
    /// all N-complement pairs aligned and `validate` clean.
    fn flush_writable_entries(d: &Descriptor, buf: &mut [u8]) {
        for item in block_of(d).items {
            let Some(reg) = writable(item) else { continue };
            if reg.bit_size > 64 {
                continue;
            }
            d.encode(buf, item.name, EncodeInput::Literal(0)).unwrap();
        }
    }

    #[test]
    fn find_round_trips_for_every_kind() {
        for d in Descriptor::iter() {
            let found = Descriptor::find(d.kind()).expect("find by own kind");
            assert_eq!(found.kind(), d.kind());
            assert_eq!(found.name(), d.name());
        }
        assert!(Descriptor::find("__no_such_kind__").is_none());
    }

    #[test]
    fn find_is_case_insensitive() {
        for d in Descriptor::iter() {
            let upper: alloc::string::String =
                d.kind().chars().map(|c| c.to_ascii_uppercase()).collect();
            assert!(Descriptor::find(&upper).is_some());
        }
    }

    #[test]
    fn entries_match_block_items() {
        for d in Descriptor::iter() {
            let api: Vec<_> = d.entries().collect();
            let meta: Vec<_> = block_of(&d).items.iter().map(|i| i.name).collect();
            assert_eq!(api, meta, "{} entries", d.kind());
        }
    }

    #[test]
    fn fields_match_fieldset_when_present() {
        for d in Descriptor::iter() {
            for item in block_of(&d).items {
                match d.fieldset_of(item) {
                    Some(fs) => {
                        let api: Vec<_> = d.fields(item.name).unwrap().collect();
                        let meta: Vec<_> = fs.fields.iter().map(|f| f.name).collect();
                        assert_eq!(api, meta, "{}.{} fields", d.kind(), item.name);
                    }
                    None => assert!(d.fields(item.name).is_none()),
                }
            }
        }
    }

    #[test]
    fn describe_entry_matches_metadata() {
        for d in Descriptor::iter() {
            for item in block_of(&d).items {
                let reg = Descriptor::register_of(item).unwrap();
                let info = d.describe(item.name).unwrap();
                assert_eq!(info.byte_offset, item.byte_offset);
                assert_eq!(info.bit_offset, 0);
                assert_eq!(info.bit_size, reg.bit_size);
                assert_eq!(info.access, reg.access);
                assert!(info.enumm.is_none());
                assert_eq!(info.description, item.description);
            }
        }
    }

    #[test]
    fn describe_field_matches_metadata() {
        for d in Descriptor::iter() {
            for item in block_of(&d).items {
                let Some(fs) = d.fieldset_of(item) else {
                    continue;
                };
                for field in fs.fields {
                    let ir::BitOffset::Regular(off) = &field.bit_offset else {
                        continue;
                    };
                    let path = format!("{}.{}", item.name, field.name);
                    let info = d.describe(&path).unwrap();
                    assert_eq!(info.byte_offset, item.byte_offset);
                    assert_eq!(info.bit_offset, off.offset);
                    assert_eq!(info.bit_size, field.bit_size);
                    assert_eq!(info.enumm.is_some(), field.enumm.is_some());
                    assert_eq!(info.description, field.description);
                }
            }
        }
    }

    #[test]
    fn entry_literal_roundtrip_for_every_writable_entry() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            for item in block_of(&d).items {
                let Some(reg) = writable(item) else { continue };
                if reg.bit_size > 64 {
                    continue;
                }
                for &val in &[0u64, bit_mask(reg.bit_size)] {
                    d.encode(&mut buf, item.name, EncodeInput::Literal(val))
                        .unwrap();
                    let v = d.decode(&buf, item.name).unwrap();
                    assert_eq!(
                        decoded_to_u64(&d, item.name, v),
                        val,
                        "{}.{} literal {}",
                        d.kind(),
                        item.name,
                        val
                    );
                }
            }
        }
    }

    #[test]
    fn field_literal_roundtrip_for_every_writable_field() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            for item in block_of(&d).items {
                if writable(item).is_none() {
                    continue;
                }
                let Some(fs) = d.fieldset_of(item) else {
                    continue;
                };
                for field in fs.fields {
                    if !matches!(field.bit_offset, ir::BitOffset::Regular(_)) {
                        continue;
                    }
                    let path = format!("{}.{}", item.name, field.name);
                    for &val in &[0u64, bit_mask(field.bit_size)] {
                        d.encode(&mut buf, &path, EncodeInput::Literal(val)).unwrap();
                        let v = d.decode(&buf, &path).unwrap();
                        assert_eq!(decoded_to_u64(&d, &path, v), val, "{} = {}", path, val);
                    }
                }
            }
        }
    }

    #[test]
    fn variant_roundtrip_for_every_enum_field() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            for item in block_of(&d).items {
                if writable(item).is_none() {
                    continue;
                }
                let Some(fs) = d.fieldset_of(item) else {
                    continue;
                };
                for field in fs.fields {
                    let Some(enumm) = field.enumm.and_then(|n| d.enumm_named(n)) else {
                        continue;
                    };
                    let path = format!("{}.{}", item.name, field.name);
                    for v in enumm.variants {
                        d.encode(&mut buf, &path, EncodeInput::Variant(v.name))
                            .unwrap();
                        let got = d.decode(&buf, &path).unwrap();
                        assert_eq!(
                            decoded_to_u64(&d, &path, got),
                            v.value,
                            "{} := {}",
                            path,
                            v.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn read_only_entries_reject_writes() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            for item in block_of(&d).items {
                let reg = Descriptor::register_of(item).unwrap();
                if !matches!(reg.access, Access::Read) {
                    continue;
                }
                assert_eq!(
                    d.encode(&mut buf, item.name, EncodeInput::Literal(0)),
                    Err(EncodeError::ReadOnly),
                    "{}.{}",
                    d.kind(),
                    item.name
                );
            }
        }
    }

    #[test]
    fn out_of_range_literals_rejected() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            for item in block_of(&d).items {
                let Some(reg) = writable(item) else { continue };
                if reg.bit_size >= 64 {
                    continue;
                }
                let too_big = 1u64 << reg.bit_size;
                assert_eq!(
                    d.encode(&mut buf, item.name, EncodeInput::Literal(too_big)),
                    Err(EncodeError::OutOfRange),
                );
            }
        }
    }

    #[test]
    fn unknown_paths_and_variants_rejected() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            assert_eq!(
                d.encode(&mut buf, "__no_entry__", EncodeInput::Literal(0)),
                Err(EncodeError::NoSuchEntry)
            );
            for item in block_of(&d).items {
                if writable(item).is_none() {
                    continue;
                }
                let Some(fs) = d.fieldset_of(item) else {
                    continue;
                };
                let bad_field = format!("{}.__no_field__", item.name);
                assert_eq!(
                    d.encode(&mut buf, &bad_field, EncodeInput::Literal(0)),
                    Err(EncodeError::NoSuchField)
                );
                if let Some(field) = fs.fields.iter().find(|f| f.enumm.is_some()) {
                    let path = format!("{}.{}", item.name, field.name);
                    assert_eq!(
                        d.encode(&mut buf, &path, EncodeInput::Variant("__no_variant__")),
                        Err(EncodeError::NoSuchVariant)
                    );
                }
                break;
            }
        }
    }

    #[test]
    fn validate_passes_after_flushing_every_entry() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            flush_writable_entries(&d, &mut buf);
            assert_eq!(d.validate(&buf), Ok(()), "{}", d.kind());
        }
    }

    #[test]
    fn validate_reports_corrupted_complement_entry() {
        for d in Descriptor::iter() {
            let block = block_of(&d);
            let pair = block.items.iter().find_map(|item| {
                writable(item)?;
                let sib = block.items.iter().find(|s| is_n_complement(s.name, item.name))?;
                Some((item, sib))
            });
            let Some((item, sibling)) = pair else { continue };
            let mut buf = [0u8; BUF];
            flush_writable_entries(&d, &mut buf);
            buf[sibling.byte_offset as usize] ^= 0xff;
            assert_eq!(
                d.validate(&buf),
                Err(ValidationError::ComplementMismatch { entry: item.name })
            );
        }
    }

    #[test]
    fn validate_buffer_too_short_when_pairs_exist() {
        for d in Descriptor::iter() {
            let block = block_of(&d);
            let has_pair = block.items.iter().any(|item| {
                block.items.iter().any(|s| is_n_complement(s.name, item.name))
            });
            if !has_pair {
                continue;
            }
            assert_eq!(d.validate(&[]), Err(ValidationError::BufferTooShort));
        }
    }

    #[test]
    fn top_level_decode_and_encode_dispatch_by_kind() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            for item in block_of(&d).items {
                let Some(reg) = writable(item) else { continue };
                if reg.bit_size > 64 {
                    continue;
                }
                let path = format!("{}.{}", d.kind(), item.name);
                encode(&path, &mut buf, EncodeInput::Literal(0)).unwrap();
                let v = decode(&path, &buf).unwrap();
                assert_eq!(decoded_to_u64(&d, item.name, v), 0);
            }
        }
        assert!(decode("__nokind__.x", &[0; BUF]).is_none());
        let mut buf = [0u8; BUF];
        assert_eq!(
            encode("__nokind__.x", &mut buf, EncodeInput::Literal(0)),
            Err(EncodeError::NoSuchEntry)
        );
    }

    #[test]
    fn decode_returns_none_on_short_buffer() {
        for d in Descriptor::iter() {
            if let Some(item) = block_of(&d).items.first() {
                assert!(d.decode(&[], item.name).is_none());
            }
        }
    }

    #[test]
    fn reset_produces_buffer_that_validates() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            d.reset(&mut buf).unwrap();
            assert_eq!(d.validate(&buf), Ok(()), "{}", d.kind());
        }
    }

    #[test]
    fn reset_writes_each_declared_default() {
        for d in Descriptor::iter() {
            let mut buf = [0u8; BUF];
            d.reset(&mut buf).unwrap();
            for (entry, value) in d.nv.defaults {
                let v = d.decode(&buf, entry).unwrap();
                assert_eq!(
                    decoded_to_u64(&d, entry, v),
                    *value as u64,
                    "{}.{}",
                    d.kind(),
                    entry
                );
            }
        }
    }

    #[test]
    fn reset_is_noop_when_defaults_empty() {
        for d in Descriptor::iter() {
            if !d.nv.defaults.is_empty() {
                continue;
            }
            let mut buf = [0xa5u8; BUF];
            let before = buf;
            d.reset(&mut buf).unwrap();
            assert_eq!(buf, before, "{}", d.kind());
        }
    }

    #[test]
    fn lifecycle_find_list_default_encode_validate() {
        for d in Descriptor::iter() {
            let found = Descriptor::find(d.kind()).unwrap();
            let entries: Vec<_> = found.entries().collect();
            assert!(!entries.is_empty(), "{}", d.kind());

            let mut buf = [0u8; BUF];
            found.reset(&mut buf).unwrap();

            // Phase 1: every declared default is visible after reset.
            for entry in &entries {
                let info = found.describe(entry).unwrap();
                let Some(def) = info.default else { continue };
                let v = found.decode(&buf, entry).unwrap();
                assert_eq!(
                    decoded_to_u64(&found, entry, v),
                    def,
                    "{}.{} default after reset",
                    found.kind(),
                    entry
                );
            }

            // Phase 2: rewrite each writable entry to default+1 and confirm
            // the buffer still validates after every step.
            for entry in &entries {
                let info = found.describe(entry).unwrap();
                if matches!(info.access, Access::Read) {
                    continue;
                }
                let Some(def) = info.default else { continue };
                let max = bit_mask(info.bit_size);
                let new_val = if def == max { 0 } else { def + 1 } & max;
                found
                    .encode(&mut buf, entry, EncodeInput::Literal(new_val))
                    .unwrap();
                let got = found.decode(&buf, entry).unwrap();
                assert_eq!(decoded_to_u64(&found, entry, got), new_val);
                assert_eq!(found.validate(&buf), Ok(()));
            }
        }
    }
}
