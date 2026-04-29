# CH32V208WBU6 BLE Register Hardware Observations

## Device & Connection

- **Chip**: CH32V208WBU6 (QFN68, QingKe V4C, 144MHz, BLE 5.3)
- **Tool**: wlink (WCH-Link)
- **Connection**: USB → WCH-Link → SWDIO/SWDCLK → CH32V208WBU6
- **State at dump**: chip halted (via `wlink dump`), BLE stack idle/not yet fully initialized

---

## Base Address Verification

All four BLE peripheral blocks accessible; no bus faults observed.

| Peripheral | Address    | Status |
|------------|------------|--------|
| BLE_LLE    | 0x40024100 | ✓ accessible |
| BLE_BB     | 0x40024200 | ✓ accessible |
| BLE_RFEND  | 0x40024300 | ✓ accessible |
| BLE_AES    | 0x40025000 | ✓ accessible |

---

## Raw Register Dumps

### LLE (0x40024100)

```
+0x00 CTRL         = 0x00000000  (idle)
+0x04 CRC_INIT     = 0x00555555  ← advertising CRC init (0x555555)
+0x08 ACCESS_ADDR  = 0x8E89BED6  ← advertising access address (confirmed BLE spec)
+0x0C IRQ_MASK     = 0x00000000  (all IRQs masked — BLE idle)
+0x14 TIMING0      = 0x0000008C  (140)
+0x1C STATE_MACHINE= 0x0000006C  (108 = SLEEP state)
+0x24 TIMING2      = 0x0000008C  (140)
+0x2C TIMING3      = 0x0000003C  (60)
+0x34 TIMING4      = 0x0000008C  (140)
+0x3C TIMING5      = 0x0000003C  (60)
+0x44 TIMING6      = 0x0000008C  (140)
+0x4C TIMING7      = 0x0000006C  (108)
+0x50 TX_CTRL      = 0x00000000
+0x68 ERROR_STATUS = 0x00000000
+0x74 CONFIG       = 0x00000000
```

**STATE_MACHINE note**: value 108 (0x6C) = SLEEP. State encoding from assembly:
- 93 = CONN_RX_WAIT
- 97 = CONN_TX_PREP
- 101 = CONN_ACK_WAIT
- 105 = CONN_EVENT_CLOSING
- 107 = SLEEP_PREP
- 108 = SLEEP

### BB (0x40024200)

Chip was halted before BLE fully initialized; most config registers = 0.
Non-zero values at even/odd offsets (e.g. +0x10/+0x14) are live timer/counter registers.

```
+0x00 MAIN_CTRL    = 0x00000000  (BB_EN=0, TX_PATH_EN=0, CORE_EN=0 — not started)
+0x04 MODE_CTRL    = 0x00000000  (Idle)
+0x40 TX_PRE_DELAY = 0x00000011  (17, not 90 — pre-init default)
```

### RFEND (0x40024300)

```
+0x08 RF_ANA_CTRL  = 0x00000000
+0x0C CTRL         = 0x00000000
+0x28 CFG0         = 0x00000480  (default 0x480)
+0x44 PLL          = 0x00000000
```

### AES (0x40025000)

```
+0x00 CTRL         = 0x00000000
+0x04 STATUS       = 0x00000000  (not busy)
+0x18 DATA0        = 0x00000000
... (all zeroes at rest)
```

---

## LLE +0x08 Dual-Purpose Analysis

Hardware read: **0x8E89BED6** — the standard BLE advertising access address.

From assembly (`libwchble.a V1.40`):

**Write path** (`ll_advertise_tx`):
```asm
# a5 = &LLE base (0x40024100)
lui  a4, 0x8e89c
addi a4, a4, -298   # a4 = 0x8E89BED6
sw   a4, 8(a5)      # LLE[+0x08] = ACCESS_ADDR
lui  a4, 0x555
addi a4, a4, 1365   # a4 = 0x555555
sw   a4, 4(a5)      # LLE[+0x04] = CRC_INIT
```

**Read path** (`LLE_IRQSubHandler`, `lle_irq_process`):
```asm
# LLE_IRQSubHandler checks IRQ_MASK (+0x0C) first, then reads +0x08:
lw   a4, 12(a5)     # read IRQ_MASK
andi a4, a4, 0x4000 # test bit 14 (IRQ_SEQ_ERROR enable)
lw   a4, 8(a5)      # read LLE[+0x08]  ← same offset, used as IRQ_STATUS here
andi a4, a4, 0x4000 # test bit 14 (SEQ_ERROR flag)
sw   a3, 8(a5)      # W1C: write 0x4000 to clear

# lle_irq_process tests bits 3, 2, 1 of LLE[+0x08]:
lw   a4, 8(a5)
andi ..., 0x8       # bit 3 = IRQ_RF_SEQ_END
andi ..., 0x4       # bit 2 = IRQ_TX_DONE
andi ..., 0x2       # bit 1 = IRQ_RX_DONE
```

**Conclusion**: LLE[+0x08] is **dual-purpose**:
- Written as `ACCESS_ADDR` (full 32-bit) by TX setup code
- Read as `IRQ_STATUS` (W1C, lower 16 bits at minimum) by IRQ handlers
- The hardware likely overlaps these: writing full 32-bit sets ACCESS_ADDR; after RX/TX events, lower bits are set as status flags

This is consistent with a common link-layer design where the access address register and interrupt status share the same address space, with the upper bits holding the address and lower bits acting as status/event flags.

---

## Assembly Cross-Verification Table

| Register         | Offset | Source     | Value/Observation              | Confidence |
|------------------|--------|------------|--------------------------------|------------|
| CRC_INIT         | +0x04  | HW + ASM   | 0x555555 (adv. channel init)   | ✓ confirmed |
| ACCESS_ADDR      | +0x08  | HW + ASM   | 0x8E89BED6 (adv. access addr)  | ✓ confirmed |
| IRQ_MASK         | +0x0C  | HW + ASM   | 0x00000000 (all masked)        | ✓ confirmed |
| STATE_MACHINE    | +0x1C  | HW         | 108 = SLEEP                    | ✓ confirmed |
| TIMING0-7        | various| HW         | 140/108/60 defaults            | ✓ confirmed |
| TX_CTRL.TX_EN    | +0x50  | ASM        | bit 5 (from assembly)          | ✓ by ASM   |
| IRQ_STATUS (dual)| +0x08  | ASM        | W1C bits in same reg as AA     | ✓ by ASM   |
| BB registers     | 0x40024200 | HW     | Not yet initialized at dump    | partial    |
| RFEND.PLL        | +0x44  | partial    | Encoding formula derived       | unverified |
| AES.DATA/KEY     | various| structure  | Layout from ASM                | unverified |

---

## ch32-data Corrections Log

### Before hardware verification (initial, from reverse-doc):
- LLE[+0x04] = STATUS
- LLE[+0x08] = IRQ_STATUS

### After hardware verification + assembly cross-reference:
- LLE[+0x04] = **CRC_INIT** (0x555555 is the advertising channel CRC init value)
- LLE[+0x08] = **ACCESS_ADDR** (0x8E89BED6 is the advertising channel access address; register also doubles as IRQ_STATUS in lower bits)
- Removed orphaned `fieldset/IRQ_STATUS` entry from ble_lle_v208.yaml
- Added descriptive notes in YAML about hardware-confirmed values

### PR
PR #31: https://github.com/ch32-rs/ch32-data/pull/31

---

## Open Questions

1. **LLE[+0x08] exact bit layout**: Upper 16 bits = ACCESS_ADDR MSB? Or full 32-bit overlap? Need to set up advertising and catch the moment just after an RX event to observe lower bits non-zero.
2. **BB register layout**: Most values were 0 due to chip being halted pre-init. Need a live BLE-advertising run to capture meaningful BB state.
3. **RFEND PLL values**: Formula derived (int_div = offset/64000, frac = rem<<10/250) but not yet tested with a known channel.
4. **TIMING registers**: Values 140/60/108 confirmed but meaning (µs? cycles?) not yet determined.
