use std::collections::HashMap;

use anyhow::anyhow;
use chiptool::ir::IR;
use chiptool::validate;

pub struct NvDescriptors {
    pub descriptors: HashMap<String, IR>,
}

impl NvDescriptors {
    pub fn parse() -> Result<Self, anyhow::Error> {
        let mut descriptors = HashMap::new();

        for f in glob::glob("data/nv/*")? {
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

            let validate_option = validate::Options {
                allow_register_overlap: false,
                allow_field_overlap: false,
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

            descriptors.insert(ff, ir);
        }

        Ok(Self { descriptors })
    }

    pub fn write(&self) -> Result<(), anyhow::Error> {
        std::fs::create_dir_all("build/data/nv")?;

        for (name, ir) in &self.descriptors {
            let dump = serde_json::to_string_pretty(ir)?;
            std::fs::write(format!("build/data/nv/{name}.json"), dump)?;
        }
        Ok(())
    }
}
