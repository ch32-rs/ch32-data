use std::fmt::Write as _;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use chiptool::{generate, ir, transform};
use regex::Regex;

use crate::{gen_opts, stringify};

pub(crate) fn gen_peripheral(out_dir: &Path, data_dir: &Path, module: &str, version: &str) {
    println!("Generate Peripheral {} {}", module, version);

    let regs_path = data_dir
        .join("registers")
        .join(format!("{}_{}.json", module, version));

    let mut ir: ir::IR = serde_json::from_reader(
        File::open(&regs_path).unwrap_or_else(|_| panic!("open {}", regs_path.display())),
    )
    .unwrap();

    transform::expand_extends::ExpandExtends {}.run(&mut ir).unwrap();

    transform::map_names(&mut ir, |k, s| match k {
        transform::NameKind::Block => *s = s.to_string(),
        transform::NameKind::Fieldset => *s = format!("regs::{}", s),
        transform::NameKind::Enum => *s = format!("vals::{}", s),
        _ => {}
    });

    transform::sort::Sort {}.run(&mut ir).unwrap();
    transform::Sanitize {}.run(&mut ir).unwrap();

    let items = generate::render(&ir, &gen_opts()).unwrap();
    let mut file = File::create(
        out_dir
            .join("src/peripherals")
            .join(format!("{}_{}.rs", module, version)),
    )
    .unwrap();

    // Allow a few warning
    file.write_all(
        b"#![allow(clippy::missing_safety_doc)]
                #![allow(clippy::identity_op)]
                #![allow(clippy::unnecessary_cast)]
                #![allow(clippy::erasing_op)]",
    )
    .unwrap();

    let data = items.to_string().replace("] ", "]\n");

    // Remove inner attributes like #![no_std]
    let re = Regex::new("# *! *\\[.*\\]").unwrap();
    let data = re.replace_all(&data, "");
    file.write_all(data.as_bytes()).unwrap();

    let ir = crate::data::ir::IR::from_chiptool(ir);
    let mut data = String::new();
    write!(
        &mut data,
        "
                    use crate::metadata::ir::*;
                    pub(crate) static REGISTERS: IR = {};
                ",
        stringify(&ir),
    )
    .unwrap();

    let mut file = File::create(
        out_dir
            .join("src/registers")
            .join(format!("{}_{}.rs", module, version)),
    )
    .unwrap();
    file.write_all(data.as_bytes()).unwrap();
}
