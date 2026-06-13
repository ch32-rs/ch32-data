use crate::metadata::ir::{Access, Enum};

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
