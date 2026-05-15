//! Runtime descriptor overlay for non-volatile structures (Option Bytes, ESIG, ...).
//! Exposes a path-based API (`descriptor.entry[.field]`) over buffers laid out per `NvStruct`.

mod codec;
mod descriptor;
mod lifecycle;
mod types;

#[cfg(test)]
mod tests;

pub use descriptor::Descriptor;
pub use types::{EncodeError, EncodeInput, Info, ValidationError, Value};

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
