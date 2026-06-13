use crate::metadata::{
    ir::{self},
    MemoryRegion, NvStruct, METADATA,
};

use super::types::Info;

#[derive(Debug, Clone, Copy)]
pub struct Descriptor {
    pub(super) region: &'static MemoryRegion,
    pub(super) nv: &'static NvStruct,
}

impl Descriptor {
    pub fn iter() -> impl Iterator<Item = Descriptor> {
        METADATA.nv_structs.iter().flat_map(|binding| {
            let region = METADATA
                .memory
                .iter()
                .find(|r| r.name == binding.region)
                .expect("nv_struct binding must reference an existing memory region");
            binding
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

    pub(super) fn block(&self) -> Option<&'static ir::Block> {
        self.nv.ir.blocks.iter().find(|b| b.name == self.nv.block)
    }

    pub(super) fn item(&self, entry: &str) -> Option<&'static ir::BlockItem> {
        self.block()?
            .items
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case(entry))
    }

    pub(super) fn register_of(item: &ir::BlockItem) -> Option<&ir::Register> {
        match &item.inner {
            ir::BlockItemInner::Register(r) => Some(r),
            _ => None,
        }
    }

    pub(super) fn fieldset_of(&self, item: &ir::BlockItem) -> Option<&'static ir::FieldSet> {
        let reg = Self::register_of(item)?;
        let name = reg.fieldset?;
        self.nv.ir.fieldsets.iter().find(|fs| fs.name == name)
    }

    pub(super) fn field_in(fs: &'static ir::FieldSet, name: &str) -> Option<&'static ir::Field> {
        fs.fields.iter().find(|f| f.name.eq_ignore_ascii_case(name))
    }

    pub(super) fn enumm_named(&self, name: &str) -> Option<&'static ir::Enum> {
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
}
