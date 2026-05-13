use crate::metadata::ir::Access;

use super::codec::is_n_complement;
use super::descriptor::Descriptor;
use super::types::{EncodeError, EncodeInput, ValidationError};

impl Descriptor {
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
