use crate::metadata::ir::{self, Access};

use super::descriptor::Descriptor;
use super::types::{EncodeError, EncodeInput, Value};

impl Descriptor {
    pub fn decode<'a>(&self, buf: &'a [u8], path: &str) -> Option<Value<'a>> {
        let info = self.describe(path)?;
        let start = info.byte_offset as usize;

        if info.bit_size > 64 {
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
            None => write_entry(
                &mut buf[entry_start..entry_start + entry_size],
                reg.bit_size,
                input,
            )?,
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

    pub(super) fn apply_complement(
        &self,
        buf: &mut [u8],
        item: &ir::BlockItem,
    ) -> Result<(), EncodeError> {
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
}

pub(super) fn is_n_complement(sibling: &str, source: &str) -> bool {
    let s = sibling.as_bytes();
    let src = source.as_bytes();
    s.len() == src.len() + 1
        && s[0].eq_ignore_ascii_case(&b'N')
        && s[1..].eq_ignore_ascii_case(src)
}

pub(super) fn bit_mask(bit_size: u32) -> u64 {
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
        // variants only resolve at field level
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
        // raw byte writes only apply at entry level
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
