// Usage:
//   cargo run -p ch32-metapac-gen --bin dump-memory-x -- [CHIP|GLOB]... \
//       [--features memory-config-c160_r32,memory-split-usr]
// Without --features, emits every (memory-config-* x subset(memory-split-*)) combination.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;

use ch32_metapac_gen::dump::{ChipDump, ChipOption, list_chips, load_chip, split_prefixes_from_names};
use ch32_metapac_gen::memory_x_render::{Region, render_memory_x, resolve_regions};

struct Cli {
    chips: Vec<String>,
    features: Option<Vec<String>>,
}

fn parse_cli(args: &[String]) -> Cli {
    let mut chips = Vec::new();
    let mut features: Option<Vec<String>> = None;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if a == "--features" || a == "-f" {
            let v = args
                .get(i + 1)
                .unwrap_or_else(|| panic!("{} requires a value", a));
            features = Some(parse_feature_list(v));
            i += 2;
            continue;
        }
        if let Some(rest) = a.strip_prefix("--features=") {
            features = Some(parse_feature_list(rest));
            i += 1;
            continue;
        }
        if a.starts_with('-') {
            panic!("Unknown flag: {} (recognized: --features/-f)", a);
        }
        chips.push(a.clone());
        i += 1;
    }
    Cli { chips, features }
}

fn parse_feature_list(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .map(|t| t.trim().to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

fn resolve_features(features: &[String]) -> (Option<String>, BTreeSet<String>) {
    let mut option: Option<String> = None;
    let mut splits: BTreeSet<String> = BTreeSet::new();
    for f in features {
        if f == "memory-x" {
            continue;
        }
        if let Some(name) = f.strip_prefix("memory-config-") {
            if let Some(prev) = option.as_ref() {
                panic!(
                    "Multiple memory-config-* features given: memory-config-{}, memory-config-{}",
                    prev, name
                );
            }
            option = Some(name.to_string());
        } else if let Some(prefix) = f.strip_prefix("memory-split-") {
            splits.insert(prefix.to_string());
        } else {
            panic!("Unknown feature: {}", f);
        }
    }
    (option, splits)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cli = parse_cli(&args);
    let data_dir = PathBuf::from("build/data");
    let out_root = PathBuf::from("build/memory-x-dump");

    if !data_dir.join("chips").exists() {
        panic!(
            "{} not found. Run `./d gen` first to populate chip data.",
            data_dir.join("chips").display()
        );
    }

    let all_chips = list_chips(&data_dir);
    let chips = select_chips(&cli.chips, &all_chips);

    let _ = fs::remove_dir_all(&out_root);
    fs::create_dir_all(&out_root).unwrap();

    let mode = match cli.features.as_deref() {
        Some(f) => format!("features=[{}]", f.join(", ")),
        None => "all variants".to_string(),
    };
    println!(
        "dumping {} chip(s) ({}) -> {}",
        chips.len(),
        mode,
        out_root.display()
    );
    for chip in &chips {
        let dump = load_chip(&data_dir, chip);
        let out_path = out_root.join(format!("{}.memory.x", chip.to_ascii_lowercase()));
        let body = match cli.features.as_deref() {
            Some(features) => render_chip_features(&dump, features),
            None => render_chip_all(&dump),
        };
        fs::write(&out_path, body).unwrap();
        println!("  {} -> {}", chip, out_path.display());
    }
}

fn select_chips(args: &[String], all_chips: &[String]) -> Vec<String> {
    if args.is_empty() {
        if all_chips.is_empty() {
            panic!("No chips found under build/data/chips");
        }
        return all_chips.to_vec();
    }
    let upper: Vec<String> = all_chips.iter().map(|c| c.to_ascii_uppercase()).collect();
    let mut chips = Vec::new();
    for arg in args {
        let arg_uc = arg.to_ascii_uppercase();
        if let Some(idx) = upper.iter().position(|c| c == &arg_uc) {
            chips.push(all_chips[idx].clone());
        } else if let Some(prefix) = arg_uc.strip_suffix('*') {
            if prefix.contains('*') {
                panic!("Only trailing `*` is supported in chip globs: {}", arg);
            }
            for (i, c) in upper.iter().enumerate() {
                if c.starts_with(prefix) {
                    chips.push(all_chips[i].clone());
                }
            }
        } else {
            panic!("Unknown chip: {}", arg);
        }
    }
    chips.sort();
    chips.dedup();
    if chips.is_empty() {
        panic!("No chips matched");
    }
    chips
}

fn feature_label(option: &str, default: &str, splits: &BTreeSet<String>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if option != default {
        parts.push(format!("memory-config-{}", option));
    }
    for p in splits {
        parts.push(format!("memory-split-{}", p));
    }
    if parts.is_empty() {
        "(no memory-config-* / memory-split-* features — default)".to_string()
    } else {
        parts.join(", ")
    }
}

fn render_variant(regions: &[Region], splits: &BTreeSet<String>, label: &str) -> String {
    let resolved = resolve_regions(regions, splits);
    let memory_x = render_memory_x(&resolved);
    let bar = "=".repeat(72);
    let mut out = String::new();
    out.push_str(&format!("# {}\n", bar));
    out.push_str(&format!("# features: {}\n", label));
    out.push_str(&format!("# {}\n\n", bar));
    out.push_str(&memory_x);
    out
}

fn find_option<'a>(dump: &'a ChipDump, name: &str) -> &'a ChipOption {
    dump.options
        .iter()
        .find(|o| o.name == name)
        .unwrap_or_else(|| {
            let avail: Vec<&str> = dump.options.iter().map(|o| o.name.as_str()).collect();
            panic!(
                "Chip {} has no memory-config-{} (available: {})",
                dump.name,
                name,
                avail.join(", "),
            )
        })
}

fn render_chip_features(dump: &ChipDump, features: &[String]) -> String {
    let (opt_arg, splits) = resolve_features(features);
    let option = match opt_arg {
        Some(o) => find_option(dump, &o),
        None => find_option(dump, &dump.default_option),
    };
    let label = feature_label(&option.name, &dump.default_option, &splits);
    render_variant(&option.regions, &splits, &label)
}

fn render_chip_all(dump: &ChipDump) -> String {
    let mut out = String::new();
    let mut options: Vec<&ChipOption> = dump.options.iter().collect();
    options.sort_by_key(|o| (o.name != dump.default_option, o.name.clone()));
    for opt in options {
        let prefixes = split_prefixes_from_names(opt.regions.iter().map(|r| r.name.as_str()));
        let n = prefixes.len();
        for mask in 0..(1u32 << n) {
            let mut splits: BTreeSet<String> = BTreeSet::new();
            for (i, p) in prefixes.iter().enumerate() {
                if mask & (1u32 << i) != 0 {
                    splits.insert(p.clone());
                }
            }
            let label = feature_label(&opt.name, &dump.default_option, &splits);
            out.push_str(&render_variant(&opt.regions, &splits, &label));
            out.push('\n');
        }
    }
    out
}
