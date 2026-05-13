//! Runtime descriptor overlay for non-volatile flash structures (Option Bytes, ESIG, ...).
//!
//! Discovers `NvStruct` metadata attached to the active chip's memory regions, then exposes
//! a path-based API (`descriptor.entry[.field]`) for reading, writing, and validating buffers
//! laid out per those descriptors.

use crate::metadata::{
    ir::{Access, Enum},
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
        Self::iter().find(|d| d.nv.kind == name)
    }

    pub fn name(&self) -> &'static str {
        self.nv.name
    }

    pub fn kind(&self) -> &'static str {
        self.nv.kind
    }

    pub fn entries(&self) -> impl Iterator<Item = &'static str> {
        let _ = self;
        unimplemented!();
        #[allow(unreachable_code)]
        core::iter::empty()
    }

    pub fn fields(&self, _entry: &str) -> Option<impl Iterator<Item = &'static str>> {
        let _ = self;
        unimplemented!();
        #[allow(unreachable_code)]
        Some(core::iter::empty())
    }

    pub fn describe(&self, _path: &str) -> Option<Info> {
        unimplemented!()
    }

    pub fn decode<'a>(&self, _buf: &'a [u8], _path: &str) -> Option<Value<'a>> {
        unimplemented!()
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

pub fn decode<'a>(_path: &str, _buf: &'a [u8]) -> Option<Value<'a>> {
    unimplemented!()
}

pub fn encode(_path: &str, _buf: &mut [u8], _input: EncodeInput) -> Result<(), EncodeError> {
    unimplemented!()
}
