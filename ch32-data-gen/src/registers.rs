use std::collections::HashMap;

use anyhow::anyhow;
use chiptool::ir::IR;
use chiptool::validate;

pub struct Registers {
    pub registers: HashMap<String, IR>,
}

#[cfg(test)]
mod tests {
    use chiptool::ir::{BitOffset, BlockItemInner, IR};

    #[test]
    fn otg_v2_host_matches_wch_sdk() {
        // WCH ch32v30x.h USBFSH_TypeDef and ch32v30x_usb.h USBFS_UH_*.
        // Generic IR validation cannot detect a valid but incorrect MMIO address.
        let ir: IR = serde_yaml::from_str(include_str!("../../data/registers/otg_v2.yaml")).unwrap();
        let host = &ir.blocks["USBH"];
        for (name, offset, width) in [
            ("RX_DMA", 0x18, 32),
            ("TX_DMA", 0x1c, 32),
            ("SETUP", 0x36, 16),
            ("EP_PID", 0x38, 8),
            ("RX_CTRL", 0x3b, 8),
            ("TX_LEN", 0x3c, 16),
            ("TX_CTRL", 0x3e, 8),
        ] {
            let item = host.items.iter().find(|item| item.name == name).unwrap();
            assert_eq!(item.byte_offset, offset, "{name} offset");
            let BlockItemInner::Register(reg) = &item.inner else {
                panic!("{name} must be a register");
            };
            assert_eq!(reg.bit_size, width, "{name} access width");
            if name == "SETUP" {
                assert_eq!(reg.fieldset.as_deref(), Some("UH_SETUP"));
            }
        }

        let setup = &ir.fieldsets["UH_SETUP"];
        assert_eq!(setup.bit_size, 16);
        for (name, offset) in [("SOF_EN", 2), ("PRE_PID_EN", 10)] {
            let field = setup.fields.iter().find(|field| field.name == name).unwrap();
            assert_eq!(field.bit_offset, BitOffset::Regular(offset), "{name} bit");
            assert_eq!(field.bit_size, 1, "{name} width");
        }
    }
}

impl Registers {
    pub fn parse() -> Result<Self, anyhow::Error> {
        let mut registers = HashMap::new();

        for f in glob::glob("data/registers/*")? {
            let f = f?;
            let ff = f
                .file_name()
                .unwrap()
                .to_string_lossy()
                .strip_suffix(".yaml")
                .unwrap()
                .to_string();
            let ir: IR = serde_yaml::from_str(&std::fs::read_to_string(&f)?)
                .map_err(|e| anyhow!("failed to parse {f:?}: {e:?}"))?;

            // validate yaml file
            // we allow register overlap and field overlap for now
            let validate_option = validate::Options {
                allow_register_overlap: true,
                allow_field_overlap: true,
                allow_enum_dup_value: false,
                allow_unused_enums: false,
                allow_unused_fieldsets: false,
            };
            let err_vec = validate::validate(&ir, validate_option);
            let err_string = err_vec.iter().fold(String::new(), |mut acc, cur| {
                acc.push_str(cur);
                acc.push('\n');
                acc
            });

            if !err_string.is_empty() {
                return Err(anyhow!(format!("\n{ff}:\n{err_string}")));
            }

            registers.insert(ff, ir);
        }

        Ok(Self { registers })
    }

    pub fn write(&self) -> Result<(), anyhow::Error> {
        std::fs::create_dir_all("build/data/registers")?;

        for (name, ir) in &self.registers {
            let dump = serde_json::to_string_pretty(ir)?;
            std::fs::write(format!("build/data/registers/{name}.json"), dump)?;
        }
        Ok(())
    }
}
