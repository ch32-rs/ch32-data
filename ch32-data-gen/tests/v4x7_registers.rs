use std::path::Path;

use serde_yaml::Value;

fn data(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../data")
        .join(path);
    serde_yaml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn named<'a>(values: &'a Value, name: &str) -> &'a Value {
    values
        .as_sequence()
        .unwrap()
        .iter()
        .find(|v| v["name"] == name)
        .unwrap()
}

fn field<'a>(doc: &'a Value, set: &str, name: &str) -> &'a Value {
    named(&doc[format!("fieldset/{set}")]["fields"], name)
}

fn bits(doc: &Value, set: &str, name: &str) -> (u64, u64) {
    let f = field(doc, set, name);
    (
        f["bit_offset"].as_u64().unwrap(),
        f["bit_size"].as_u64().unwrap(),
    )
}

fn mask(doc: &Value, set: &str) -> u32 {
    doc[format!("fieldset/{set}")]["fields"]
        .as_sequence()
        .unwrap()
        .iter()
        .fold(0, |mask, f| {
            let offset = f["bit_offset"].as_u64().unwrap();
            let size = f["bit_size"].as_u64().unwrap();
            mask | ((((1_u64 << size) - 1) << offset) as u32)
        })
}

// Keep the project's overlapping 64-bit/high-word views; SWIE belongs to SR.
#[test]
fn systick_preserves_extended_access_views_and_correct_interrupt_fields() {
    let d = data("registers/systick_v4x7.yaml");
    let items = &d["block/SYSTICK"]["items"];
    assert_eq!(items.as_sequence().unwrap().len(), 8);
    for (name, offset) in [
        ("CTLR", 0),
        ("SR", 4),
        ("CNTL", 8),
        ("CNTH", 12),
        ("CMPL", 16),
        ("CMPH", 20),
    ] {
        let item = named(items, name);
        assert_eq!(item["byte_offset"].as_u64(), Some(offset));
        assert_eq!(item["bit_size"].as_u64().unwrap_or(32), 32);
    }
    for (name, low, high) in [("CNT", "CNTL", "CNTH"), ("CMP", "CMPL", "CMPH")] {
        let item = named(items, name);
        let offset = item["byte_offset"].as_u64().unwrap();
        assert_eq!(item["bit_size"].as_u64(), Some(64));
        assert_eq!(named(items, low)["byte_offset"].as_u64(), Some(offset));
        assert_eq!(named(items, high)["byte_offset"].as_u64(), Some(offset + 4));
    }
    assert_eq!(mask(&d, "CTLR"), 0x3f);
    assert_eq!(mask(&d, "SR"), 0x8000_0001);
    assert_eq!(bits(&d, "SR", "SWIE"), (31, 1));
}

// Sections 9.4.5.38..43: single-core registers, full event mask and no RSTEN.
#[test]
fn pfic_exposes_single_core_events_and_all_priority_bytes() {
    let d = data("registers/pfic_v4x7.yaml");
    let items = &d["block/PFIC"]["items"];
    for (name, offset, access) in [
        ("WAKEIP0", 0x720, None),
        ("CSTAR0", 0x780, Some("Read")),
        ("EENR", 0xc80, None),
        ("EPR", 0xc84, None),
        ("EWUPR", 0xc88, Some("Read")),
    ] {
        let item = named(items, name);
        assert_eq!(item["byte_offset"].as_u64(), Some(offset));
        assert_eq!(item["access"].as_str(), access);
    }
    assert!(!items
        .as_sequence()
        .unwrap()
        .iter()
        .any(|i| { matches!(i["name"].as_str(), Some("WAKEIP1" | "CSTAR1")) }));
    let priority = named(items, "IPRIOR");
    assert_eq!(priority["byte_offset"].as_u64(), Some(0x400));
    assert_eq!(priority["bit_size"].as_u64(), Some(8));
    assert_eq!(priority["array"]["len"].as_u64(), Some(256));
    assert_eq!(priority["array"]["stride"].as_u64(), Some(1));
    assert_eq!(bits(&d, "WAKEIP0", "IP_RELOAD0"), (1, 31));
    assert_eq!(bits(&d, "EENR", "EVENTEN"), (0, 32));
    assert_eq!(bits(&d, "EPR", "EVENT_PEND7_0"), (0, 8));
    assert_eq!(bits(&d, "EPR", "EVENT_PEND31_8"), (8, 24));
    assert_eq!(bits(&d, "EWUPR", "EVENT_WUP"), (0, 32));
    assert_eq!(mask(&d, "CSTAR0"), 0xfbff);
    assert_eq!(mask(&d, "GISR"), 0xfbff);
    assert_eq!(mask(&d, "SCTLR"), 0x80ff_003e);
    assert_eq!(named(items, "CFGR")["access"], "Write");
    let keys = d["enum/KEYCODE"]["variants"].as_sequence().unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0]["value"].as_u64(), Some(0xbeef));
}

#[test]
fn pfic_status_and_control_masks_do_not_include_reserved_holes() {
    let d = data("registers/pfic_v4x7.yaml");
    for set in ["ISR1", "IPR1", "IPSR1", "IPRR1"] {
        assert_eq!(mask(&d, set), 0xffff_532c, "{set}");
    }
    for set in ["IENR1", "IRER1"] {
        assert_eq!(mask(&d, set), 0xffff_5000, "{set}");
    }
    assert_eq!(mask(&d, "IACTR1"), 0xffff_500c);
    let items = &d["block/PFIC"]["items"];
    for n in 1..=4 {
        assert_eq!(named(items, &format!("IACTR{n}"))["access"], "Read");
    }
}

// Section 9.4.4: bits 31:22 are reserved in every EXTI register.
#[test]
fn exti_has_exactly_twenty_two_lines() {
    let d = data("registers/exti_v4x7.yaml");
    let items = d["block/EXTI"]["items"].as_sequence().unwrap();
    assert_eq!(items.len(), 6);
    for (index, item) in items.iter().enumerate() {
        assert_eq!(item["byte_offset"].as_u64(), Some(index as u64 * 4));
        let set = item["fieldset"].as_str().unwrap();
        let fields = d[format!("fieldset/{set}")]["fields"]
            .as_sequence()
            .unwrap();
        assert_eq!(fields.len(), 1);
        let f = &fields[0];
        assert_eq!(f["bit_offset"].as_u64(), Some(0));
        assert_eq!(f["bit_size"].as_u64(), Some(1));
        assert_eq!(f["array"]["len"].as_u64(), Some(22));
        assert_eq!(f["array"]["stride"].as_u64(), Some(1));
    }
}

// Sections 22.2.1.5 and 22.2.2.14/15 override the narrower vendor SVD fields.
#[test]
fn usbhs_wake_access_does_not_overlap_test_mode_or_truncate_dma_addresses() {
    let d = data("registers/usbhs_v4x7.yaml");
    let items = &d["block/USB"]["items"];
    let wake = named(items, "USB_WAKE_CTRL");
    assert_eq!(wake["byte_offset"].as_u64(), Some(4));
    assert_eq!(wake["bit_size"].as_u64(), Some(8));
    assert_eq!(d["fieldset/USB_WAKE_CTRL"]["bit_size"].as_u64(), Some(8));
    assert_eq!(
        named(items, "USB_TEST_MODE")["byte_offset"].as_u64(),
        Some(5)
    );
    for name in ["UH_RX_DMA", "UH_TX_DMA"] {
        assert_eq!(bits(&d, name, name), (0, 32));
    }
}

#[test]
fn family_selects_the_corrected_register_versions() {
    let family = data("family/CH32V4X7.yaml");
    for name in ["PFIC", "SYSTICK", "EXTI"] {
        assert_eq!(named(&family, name)["registers"]["version"], "v4x7");
    }
}
