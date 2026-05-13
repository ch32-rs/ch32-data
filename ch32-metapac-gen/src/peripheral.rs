use std::fmt::Write as _;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use chiptool::{generate, ir, transform};
use regex::Regex;

use crate::{gen_opts, stringify};

fn load_and_normalize_ir(json_path: &Path, sanitize: bool) -> ir::IR {
    let mut ir: ir::IR = serde_json::from_reader(
        File::open(json_path).unwrap_or_else(|_| panic!("open {}", json_path.display())),
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
    // Sanitize mangles names; NV consumers need RM names verbatim.
    if sanitize {
        transform::Sanitize {}.run(&mut ir).unwrap();
    }

    ir
}

fn emit_registers_rs(
    out_dir: &Path,
    module: &str,
    version: &str,
    ir: &ir::IR,
    const_name: &str,
) {
    let our_ir = crate::data::ir::IR::from_chiptool(ir.clone());
    let mut data = String::new();
    write!(
        &mut data,
        "
                    use crate::metadata::ir::*;
                    pub(crate) static {}: IR = {};
                ",
        const_name,
        stringify(&our_ir),
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

fn registers_json_path(data_dir: &Path, subdir: &str, module: &str, version: &str) -> PathBuf {
    data_dir
        .join(subdir)
        .join(format!("{}_{}.json", module, version))
}

pub(crate) fn gen_peripheral(out_dir: &Path, data_dir: &Path, module: &str, version: &str) {
    println!("Generate Peripheral {} {}", module, version);

    let json_path = registers_json_path(data_dir, "registers", module, version);
    let ir = load_and_normalize_ir(&json_path, true);

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

    emit_registers_rs(out_dir, module, version, &ir, "REGISTERS");
}

pub(crate) fn gen_nv(out_dir: &Path, data_dir: &Path, module: &str, version: &str) {
    println!("Generate NV {} {}", module, version);
    let json_path = registers_json_path(data_dir, "nv", module, version);
    let ir = load_and_normalize_ir(&json_path, false);
    emit_registers_rs(out_dir, module, version, &ir, "DESCRIPTOR");
}
