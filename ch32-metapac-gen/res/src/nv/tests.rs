extern crate alloc;

use alloc::format;
use alloc::vec::Vec;

use crate::metadata::ir::{self, Access};

use super::codec::{bit_mask, is_n_complement};
use super::*;

const BUF: usize = 256;

fn block_of(d: &Descriptor) -> &'static ir::Block {
    d.block().expect("descriptor has block")
}

fn writable(item: &'static ir::BlockItem) -> Option<&'static ir::Register> {
    match Descriptor::register_of(item) {
        Some(r) if !matches!(r.access, Access::Read) => Some(r),
        _ => None,
    }
}

fn decoded_to_u64(d: &Descriptor, path: &str, v: Value) -> u64 {
    match v {
        Value::Literal(x) => x,
        Value::Variant(name) => {
            let info = d.describe(path).unwrap();
            let enumm = info.enumm.expect("Value::Variant requires an enum");
            enumm
                .variants
                .iter()
                .find(|v| v.name == name)
                .unwrap()
                .value
        }
        Value::Bytes(_) => panic!("unexpected Value::Bytes for {}", path),
    }
}

/// Pre-fill `buf` so every writable entry has been encoded once; this leaves
/// all N-complement pairs aligned and `validate` clean.
fn flush_writable_entries(d: &Descriptor, buf: &mut [u8]) {
    for item in block_of(d).items {
        let Some(reg) = writable(item) else { continue };
        if reg.bit_size > 64 {
            continue;
        }
        d.encode(buf, item.name, EncodeInput::Literal(0)).unwrap();
    }
}

#[test]
fn find_round_trips_for_every_kind() {
    for d in Descriptor::iter() {
        let found = Descriptor::find(d.kind()).expect("find by own kind");
        assert_eq!(found.kind(), d.kind());
        assert_eq!(found.name(), d.name());
    }
    assert!(Descriptor::find("__no_such_kind__").is_none());
}

#[test]
fn find_is_case_insensitive() {
    for d in Descriptor::iter() {
        let upper: alloc::string::String =
            d.kind().chars().map(|c| c.to_ascii_uppercase()).collect();
        assert!(Descriptor::find(&upper).is_some());
    }
}

#[test]
fn entries_match_block_items() {
    for d in Descriptor::iter() {
        let api: Vec<_> = d.entries().collect();
        let meta: Vec<_> = block_of(&d).items.iter().map(|i| i.name).collect();
        assert_eq!(api, meta, "{} entries", d.kind());
    }
}

#[test]
fn fields_match_fieldset_when_present() {
    for d in Descriptor::iter() {
        for item in block_of(&d).items {
            match d.fieldset_of(item) {
                Some(fs) => {
                    let api: Vec<_> = d.fields(item.name).unwrap().collect();
                    let meta: Vec<_> = fs.fields.iter().map(|f| f.name).collect();
                    assert_eq!(api, meta, "{}.{} fields", d.kind(), item.name);
                }
                None => assert!(d.fields(item.name).is_none()),
            }
        }
    }
}

#[test]
fn describe_entry_matches_metadata() {
    for d in Descriptor::iter() {
        for item in block_of(&d).items {
            let reg = Descriptor::register_of(item).unwrap();
            let info = d.describe(item.name).unwrap();
            assert_eq!(info.byte_offset, item.byte_offset);
            assert_eq!(info.bit_offset, 0);
            assert_eq!(info.bit_size, reg.bit_size);
            assert_eq!(info.access, reg.access);
            assert!(info.enumm.is_none());
            assert_eq!(info.description, item.description);
        }
    }
}

#[test]
fn describe_field_matches_metadata() {
    for d in Descriptor::iter() {
        for item in block_of(&d).items {
            let Some(fs) = d.fieldset_of(item) else {
                continue;
            };
            for field in fs.fields {
                let ir::BitOffset::Regular(off) = &field.bit_offset else {
                    continue;
                };
                let path = format!("{}.{}", item.name, field.name);
                let info = d.describe(&path).unwrap();
                assert_eq!(info.byte_offset, item.byte_offset);
                assert_eq!(info.bit_offset, off.offset);
                assert_eq!(info.bit_size, field.bit_size);
                assert_eq!(info.enumm.is_some(), field.enumm.is_some());
                assert_eq!(info.description, field.description);
            }
        }
    }
}

#[test]
fn entry_literal_roundtrip_for_every_writable_entry() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        for item in block_of(&d).items {
            let Some(reg) = writable(item) else { continue };
            if reg.bit_size > 64 {
                continue;
            }
            for &val in &[0u64, bit_mask(reg.bit_size)] {
                d.encode(&mut buf, item.name, EncodeInput::Literal(val))
                    .unwrap();
                let v = d.decode(&buf, item.name).unwrap();
                assert_eq!(
                    decoded_to_u64(&d, item.name, v),
                    val,
                    "{}.{} literal {}",
                    d.kind(),
                    item.name,
                    val
                );
            }
        }
    }
}

#[test]
fn field_literal_roundtrip_for_every_writable_field() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        for item in block_of(&d).items {
            if writable(item).is_none() {
                continue;
            }
            let Some(fs) = d.fieldset_of(item) else {
                continue;
            };
            for field in fs.fields {
                if !matches!(field.bit_offset, ir::BitOffset::Regular(_)) {
                    continue;
                }
                let path = format!("{}.{}", item.name, field.name);
                for &val in &[0u64, bit_mask(field.bit_size)] {
                    d.encode(&mut buf, &path, EncodeInput::Literal(val)).unwrap();
                    let v = d.decode(&buf, &path).unwrap();
                    assert_eq!(decoded_to_u64(&d, &path, v), val, "{} = {}", path, val);
                }
            }
        }
    }
}

#[test]
fn variant_roundtrip_for_every_enum_field() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        for item in block_of(&d).items {
            if writable(item).is_none() {
                continue;
            }
            let Some(fs) = d.fieldset_of(item) else {
                continue;
            };
            for field in fs.fields {
                let Some(enumm) = field.enumm.and_then(|n| d.enumm_named(n)) else {
                    continue;
                };
                let path = format!("{}.{}", item.name, field.name);
                for v in enumm.variants {
                    d.encode(&mut buf, &path, EncodeInput::Variant(v.name))
                        .unwrap();
                    let got = d.decode(&buf, &path).unwrap();
                    assert_eq!(
                        decoded_to_u64(&d, &path, got),
                        v.value,
                        "{} := {}",
                        path,
                        v.name
                    );
                }
            }
        }
    }
}

#[test]
fn read_only_entries_reject_writes() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        for item in block_of(&d).items {
            let reg = Descriptor::register_of(item).unwrap();
            if !matches!(reg.access, Access::Read) {
                continue;
            }
            assert_eq!(
                d.encode(&mut buf, item.name, EncodeInput::Literal(0)),
                Err(EncodeError::ReadOnly),
                "{}.{}",
                d.kind(),
                item.name
            );
        }
    }
}

#[test]
fn out_of_range_literals_rejected() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        for item in block_of(&d).items {
            let Some(reg) = writable(item) else { continue };
            if reg.bit_size >= 64 {
                continue;
            }
            let too_big = 1u64 << reg.bit_size;
            assert_eq!(
                d.encode(&mut buf, item.name, EncodeInput::Literal(too_big)),
                Err(EncodeError::OutOfRange),
            );
        }
    }
}

#[test]
fn unknown_paths_and_variants_rejected() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        assert_eq!(
            d.encode(&mut buf, "__no_entry__", EncodeInput::Literal(0)),
            Err(EncodeError::NoSuchEntry)
        );
        for item in block_of(&d).items {
            if writable(item).is_none() {
                continue;
            }
            let Some(fs) = d.fieldset_of(item) else {
                continue;
            };
            let bad_field = format!("{}.__no_field__", item.name);
            assert_eq!(
                d.encode(&mut buf, &bad_field, EncodeInput::Literal(0)),
                Err(EncodeError::NoSuchField)
            );
            if let Some(field) = fs.fields.iter().find(|f| f.enumm.is_some()) {
                let path = format!("{}.{}", item.name, field.name);
                assert_eq!(
                    d.encode(&mut buf, &path, EncodeInput::Variant("__no_variant__")),
                    Err(EncodeError::NoSuchVariant)
                );
            }
            break;
        }
    }
}

#[test]
fn validate_passes_after_flushing_every_entry() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        flush_writable_entries(&d, &mut buf);
        assert_eq!(d.validate(&buf), Ok(()), "{}", d.kind());
    }
}

#[test]
fn validate_reports_corrupted_complement_entry() {
    for d in Descriptor::iter() {
        let block = block_of(&d);
        let pair = block.items.iter().find_map(|item| {
            writable(item)?;
            let sib = block.items.iter().find(|s| is_n_complement(s.name, item.name))?;
            Some((item, sib))
        });
        let Some((item, sibling)) = pair else { continue };
        let mut buf = [0u8; BUF];
        flush_writable_entries(&d, &mut buf);
        buf[sibling.byte_offset as usize] ^= 0xff;
        assert_eq!(
            d.validate(&buf),
            Err(ValidationError::ComplementMismatch { entry: item.name })
        );
    }
}

#[test]
fn validate_buffer_too_short_when_pairs_exist() {
    for d in Descriptor::iter() {
        let block = block_of(&d);
        let has_pair = block.items.iter().any(|item| {
            block.items.iter().any(|s| is_n_complement(s.name, item.name))
        });
        if !has_pair {
            continue;
        }
        assert_eq!(d.validate(&[]), Err(ValidationError::BufferTooShort));
    }
}

#[test]
fn top_level_decode_and_encode_dispatch_by_kind() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        for item in block_of(&d).items {
            let Some(reg) = writable(item) else { continue };
            if reg.bit_size > 64 {
                continue;
            }
            let path = format!("{}.{}", d.kind(), item.name);
            encode(&path, &mut buf, EncodeInput::Literal(0)).unwrap();
            let v = decode(&path, &buf).unwrap();
            assert_eq!(decoded_to_u64(&d, item.name, v), 0);
        }
    }
    assert!(decode("__nokind__.x", &[0; BUF]).is_none());
    let mut buf = [0u8; BUF];
    assert_eq!(
        encode("__nokind__.x", &mut buf, EncodeInput::Literal(0)),
        Err(EncodeError::NoSuchEntry)
    );
}

#[test]
fn decode_returns_none_on_short_buffer() {
    for d in Descriptor::iter() {
        if let Some(item) = block_of(&d).items.first() {
            assert!(d.decode(&[], item.name).is_none());
        }
    }
}

#[test]
fn reset_produces_buffer_that_validates() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        d.reset(&mut buf).unwrap();
        assert_eq!(d.validate(&buf), Ok(()), "{}", d.kind());
    }
}

#[test]
fn reset_writes_each_declared_default() {
    for d in Descriptor::iter() {
        let mut buf = [0u8; BUF];
        d.reset(&mut buf).unwrap();
        for (entry, value) in d.nv.defaults {
            let v = d.decode(&buf, entry).unwrap();
            assert_eq!(
                decoded_to_u64(&d, entry, v),
                *value as u64,
                "{}.{}",
                d.kind(),
                entry
            );
        }
    }
}

#[test]
fn reset_is_noop_when_defaults_empty() {
    for d in Descriptor::iter() {
        if !d.nv.defaults.is_empty() {
            continue;
        }
        let mut buf = [0xa5u8; BUF];
        let before = buf;
        d.reset(&mut buf).unwrap();
        assert_eq!(buf, before, "{}", d.kind());
    }
}

#[test]
fn lifecycle_find_list_default_encode_validate() {
    for d in Descriptor::iter() {
        let found = Descriptor::find(d.kind()).unwrap();
        let entries: Vec<_> = found.entries().collect();
        assert!(!entries.is_empty(), "{}", d.kind());

        let mut buf = [0u8; BUF];
        found.reset(&mut buf).unwrap();

        // Phase 1: every declared default is visible after reset.
        for entry in &entries {
            let info = found.describe(entry).unwrap();
            let Some(def) = info.default else { continue };
            let v = found.decode(&buf, entry).unwrap();
            assert_eq!(
                decoded_to_u64(&found, entry, v),
                def,
                "{}.{} default after reset",
                found.kind(),
                entry
            );
        }

        // Phase 2: rewrite each writable entry to default+1 and confirm
        // the buffer still validates after every step.
        for entry in &entries {
            let info = found.describe(entry).unwrap();
            if matches!(info.access, Access::Read) {
                continue;
            }
            let Some(def) = info.default else { continue };
            let max = bit_mask(info.bit_size);
            let new_val = if def == max { 0 } else { def + 1 } & max;
            found
                .encode(&mut buf, entry, EncodeInput::Literal(new_val))
                .unwrap();
            let got = found.decode(&buf, entry).unwrap();
            assert_eq!(decoded_to_u64(&found, entry, got), new_val);
            assert_eq!(found.validate(&buf), Ok(()));
        }
    }
}
