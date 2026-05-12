// regions line format: `<name> <address-hex> <size-decimal> <access:rwx> <kind:flash|ram>`

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

#[derive(Clone, Debug)]
pub struct Region {
    pub name: String,
    pub address: u32,
    pub size: u32,
    pub access: String,
    pub kind: String,
}

pub fn parse_regions(src: &str) -> Vec<Region> {
    let mut out = Vec::new();
    for line in src.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(parts.len(), 5, "malformed region line: {:?}", line);
        let addr_str = parts[1].trim_start_matches("0x");
        let address = u32::from_str_radix(addr_str, 16)
            .unwrap_or_else(|_| panic!("bad address {:?}", parts[1]));
        let size: u32 = parts[2].parse().expect("bad size");
        out.push(Region {
            name: parts[0].to_string(),
            address,
            size,
            access: parts[3].to_string(),
            kind: parts[4].to_string(),
        });
    }
    out
}

pub fn split_name(name: &str) -> Option<(&str, u32)> {
    let (prefix, suffix) = name.rsplit_once('_')?;
    if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n: u32 = suffix.parse().ok()?;
    Some((prefix, n))
}

// `split_prefixes` are lowercase by convention (both call sites lowercase before insertion).
pub fn resolve_regions(regions: &[Region], split_prefixes: &BTreeSet<String>) -> Vec<Region> {
    let mut groups: BTreeMap<String, Vec<Region>> = BTreeMap::new();
    let mut standalone: Vec<Region> = Vec::new();
    for r in regions {
        if r.size == 0 {
            continue;
        }
        match split_name(&r.name) {
            Some((prefix, _)) => groups
                .entry(prefix.to_string())
                .or_default()
                .push(r.clone()),
            None => standalone.push(r.clone()),
        }
    }

    let mut out = Vec::new();
    out.extend(standalone);
    for (prefix, mut group) in groups {
        group.sort_by_key(|r| r.address);
        // single-member groups are renamed (USR_1 -> USR); .all() is vacuously true
        let mergeable = group.windows(2).all(|w| {
            w[0].address + w[0].size == w[1].address
                && w[0].access == w[1].access
                && w[0].kind == w[1].kind
        });
        let split_forced = split_prefixes.contains(&prefix.to_ascii_lowercase());
        if mergeable && !split_forced {
            let total: u32 = group.iter().map(|r| r.size).sum();
            let first = &group[0];
            out.push(Region {
                name: prefix.clone(),
                address: first.address,
                size: total,
                access: first.access.clone(),
                kind: first.kind.clone(),
            });
        } else {
            out.extend(group);
        }
    }
    out.sort_by_key(|r| (r.kind != "flash", r.address));
    out
}

fn format_length(size: u32) -> String {
    if size >= 1024 && size % 1024 == 0 {
        format!("{:>4}K", size / 1024)
    } else {
        format!("{:>5}", size)
    }
}

fn access_attrs(access: &str) -> String {
    let trimmed: String = access.chars().filter(|c| *c != '-').collect();
    format!("({})", trimmed)
}

pub fn render_memory_x(regions: &[Region]) -> String {
    let mut s = String::new();
    let primary = regions
        .iter()
        .filter(|r| r.kind == "flash")
        .min_by_key(|r| r.address)
        .expect("no flash region after resolution");
    let ram_name = regions
        .iter()
        .find(|r| r.kind == "ram")
        .map(|r| r.name.clone())
        .expect("no ram region after resolution");
    let boot_aliased = primary.address == 0x0800_0000;

    writeln!(s, "MEMORY").unwrap();
    writeln!(s, "{{").unwrap();
    if boot_aliased {
        writeln!(
            s,
            "    {:<6} {:<5} : ORIGIN = 0x00000000, LENGTH = {} /* {} boot alias */",
            "CODE",
            "(rx)",
            format_length(primary.size),
            primary.name,
        )
        .unwrap();
    }
    for r in regions {
        writeln!(
            s,
            "    {:<6} {:<5} : ORIGIN = 0x{:08X}, LENGTH = {}",
            r.name,
            access_attrs(&r.access),
            r.address,
            format_length(r.size),
        )
        .unwrap();
    }
    writeln!(s, "}}").unwrap();
    writeln!(s).unwrap();

    let text_target = if boot_aliased { "CODE" } else { &primary.name };
    if boot_aliased {
        writeln!(s, r#"REGION_ALIAS("FLASH", CODE);"#).unwrap();
        writeln!(s).unwrap();
    }
    writeln!(s, r#"REGION_ALIAS("REGION_TEXT", {});"#, text_target).unwrap();
    writeln!(s, r#"REGION_ALIAS("REGION_RODATA", {});"#, text_target).unwrap();
    writeln!(s, r#"REGION_ALIAS("REGION_DATA", {});"#, ram_name).unwrap();
    writeln!(s, r#"REGION_ALIAS("REGION_BSS", {});"#, ram_name).unwrap();
    writeln!(s, r#"REGION_ALIAS("REGION_HEAP", {});"#, ram_name).unwrap();
    writeln!(s, r#"REGION_ALIAS("REGION_STACK", {});"#, ram_name).unwrap();
    s
}
