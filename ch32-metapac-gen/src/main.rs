use std::env::args;
use std::path::PathBuf;

use ch32_metapac_gen::*;

fn main() {
    let out_dir = PathBuf::from("build/ch32-metapac");
    let data_dir = PathBuf::from("build/data");

    let args: Vec<String> = args().collect();

    let all_chips: Vec<_> = std::fs::read_dir(data_dir.join("chips"))
        .unwrap()
        .filter_map(|res| res.unwrap().file_name().to_str().map(|s| s.to_string()))
        .filter(|s| s.ends_with(".json"))
        .map(|s| s.strip_suffix(".json").unwrap().to_string())
        .collect();

    let mut chips = match &args[..] {
        //[_, chip] => {
        //    vec![chip.clone()]
        //}
        [_] => all_chips.clone(),
        _ => {
            let mut chips = vec![];
            for arg in &args[1..] {
                if all_chips.contains(arg) {
                    chips.push(arg.clone());
                } else if arg.ends_with("*") {
                    let prefix = arg.strip_suffix("*").unwrap();
                    for chip in &all_chips {
                        if chip.starts_with(prefix) {
                            chips.push(chip.clone());
                        }
                    }
                } else {
                    println!("Unknown chip: {}", arg);
                    panic!();
                }
            }
            chips
        }
    };

    chips.sort();

    println!("chips: {:?}", chips);

    let opts = Options {
        out_dir,
        data_dir,
        chips,
    };
    Gen::new(opts).gen();
}
