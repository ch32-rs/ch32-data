# USB IP cores (FS / HS)

WCH ships several USB controllers under a small set of marketing names
(`USBD`, `USBFS`, `USBHS`, `USBSS`). The same marketing name can cover more
than one register layout, and the same layout can appear under more than one
`kind` in this repo. This document is the census: which files are the same
IP, which are versions of each other, and how a rename should look.

Canonical family ↔ `(kind, version)` mapping:
[Proposed data changes](#proposed-data-changes).

**Scope.** Full-Speed and High-Speed only. USB Power Delivery (`usbpd_*`)
is a separate PHY/protocol block and is **not** counted as a USB IP here.
USB SuperSpeed (`usbss_h4`) is listed only to park it outside FS/HS.

Sources: `data/registers/*.yaml`, `data/peripherals/*.yaml`,
`data/family/*.yaml`, the RMs under `~/Downloads` (`CH32V103RM`,
`CH32FV2x_V3xRM`, `CH32L103RM`, `CH32X035RM`, `CH32V407RM`, `CH32H417RM`),
and — for H4 naming — `CH32H417EVT/.../ch32h417.h` / `ch32h417_usb.h`.

---

## Manual names vs register IPs

What the reference manuals call the peripheral:

| RM chapter title | Instance name(s) | Manuals |
| --- | --- | --- |
| USB 全速设备控制器（**USBD**） | `USBD` | V103 §20, FV2x_V3x §21 |
| USB 全速主机/设备控制器（**USBFS**） | `USBFS` | V103 §21, L103 §20, X035 §18 |
| USB 全速主机/设备控制器（**USBFS/OTG_FS**） | `USBFS` / `OTG_FS` | FV2x_V3x §23, H417 §26 |
| USB 高速主机/设备控制器（**USBHS**） | `USBHS`, `USBHS1`/`2` | FV2x_V3x §22, V407 §22, H417 §25 |
| USB 3.0 超高速…（**USBSS**） | `USBSS` | H417 §27 only — **out of FS/HS scope** |

So the datasheet vocabulary for FS/HS is just three product names: **USBD**,
**USBFS**, **USBHS**. OTG is not a fourth product — it is the same USBFS
chapter with ID/VBUS pins and two extra control registers.

Register-level IP count for FS/HS:

| # | IP | Manual name | Distinct? |
| --- | --- | --- | --- |
| 1 | Classic device-only (EPR/BTABLE) | USBD | Yes |
| 2 | WCH FS (`UEP*` + optional OTG) | USBFS / USBFS/OTG_FS | Yes — one family, several layouts |
| 3 | USBHS V3 | USBHS | Yes — 16-endpoint bitmap layout |
| 4 | USBHS V4x7 / H4 | USBHS | Yes — 8-endpoint per-endpoint layout; H4 adds FIFO arrays |

That is **four** FS/HS controllers. USBSS would be a fifth product line;
USBPD is not in this table.

---

## How `data/` encodes them today

| Manual | `kind` | `version` | Register file | Notes |
| --- | --- | --- | --- | --- |
| USBD | `usbd` | `v2` | `usbd_v2` (+ `usbram_16x1_512`) | ST-style device; may coexist with USBFS |
| USBFS | `usb` | `v1fs` | `usb_v1fs` | V103 |
| USBFS | `usb` | `v2fs` | `usb_v2fs` | FV2x without OTG block |
| USBFS | `usb` | `l1fs` | `usb_l1fs` | L103 |
| USBFS | `usb` | `x0fs` | `usb_x0fs` | X0 / X035 |
| USBFS/OTG_FS | `otg` | `v2` | `otg_v2` | = `v2fs` device + `OTG_CR`/`OTG_SR` |
| USBFS/OTG_FS | `usbfs` | `h4` | `usbfs_h4` | Naming variant of `otg_v2` (`USB_` prefixes) |
| USBHS | `usbhs` | `v3` | `usbhs_v3` | V305/V307/V317 — own layout |
| USBHS | `usbhs` | `v4x7` | `usbhs_v4x7` | V407 USBHS1/2 — same device block as H4 |
| USBHS | `usbhs` | `h4` | `usbhs_h4` | H417 — v4x7 map plus RX/TX FIFO arrays |
| USBSS | `usbss` | `h4` | `usbss_h4` | Parked; not FS/HS |

`kind: usb` alone is therefore not an identity — only `(kind, version)` is.

---

## IP 1 — USBD (classic device)

`usbd_v2.yaml`, block `USB` at `0x40005C00` on FV2x:

```
EPR[8]@0, CNTR@0x40, ISTR@0x44, FNR@0x48, DADDR@0x4C, BTABLE@0x50
```

Packet buffer is `usbram_16x1_512` at `0x40006000`. No host view.

On CH32V203/V208 this sits **beside** USBFS (`0x50000000`); neither replaces
the other. Some V203 package variants only have one of the two — check the
chip YAML.

HAL today: `ch32-hal::usbd` gated on `usbd`.

---

## IP 2 — WCH USBFS (`UEP*`), including OTG

Shared base on every variant:

```
CTRL@0, INT_EN@2, DEV_AD@3, MIS_ST@5, INT_FG@6, INT_ST@7, RX_LEN@8
```

Same `USB_BASE_CTRL` field set (`DMA_EN`, `CLR_ALL`, `RESET_SIE`, `INT_BUSY`,
`SYS_CTRL[2]`, `DEV_PU_EN`, `LOW_SPEED`, `HOST_MODE`).

Four device layouts; OTG/H4 attach to the `v2fs` layout.

### `v1fs` — `usb_v1fs.yaml` (V103)

- 8 endpoints.
- `UEP_CTRL` is **one 8-bit** register for both directions.
- `UEP_DMA` is **16-bit** (`R16_UEPn_DMA`).
- Host: V103 layout (`SETUP@0x36` 8-bit, `RX_CTRL@0x3A`).

### `v2fs` — `usb_v2fs.yaml` (V203/V208/V303 USBFS)

- 8 endpoints.
- `UEP_CTRL` split into `UEP_TX_CTRL@0x32` + `UEP_RX_CTRL@0x33`.
- Host: FV2x layout (`SETUP@0x36` 16-bit, `RX_CTRL@0x3B`, `TX_LEN@0x3C`).

### `l1fs` — `usb_l1fs.yaml` (L103)

- 8 endpoints.
- `UEP_CTRL` is **one 16-bit** register (TX low byte, RX high byte).
- `UEP_DMA` 15-bit valid (`[14:0]`); `UEP_T_LEN` 16-bit.
- Host block matches FV2x, not V103.
- Extra `DETECT` / `ONE_WIRE` on `INT_EN`.

### `x0fs` — `usb_x0fs.yaml` (X0 / X035)

- **5 endpoints**, registers grouped into range arrays
  (`UEP0123_DMA`, `UEP567_*`, …).
- Host registers shifted (`SETUP@0x26` …); `TX_LEN` 8-bit.
- Host bit layout still unverified against an X0 manual (currently matches
  V103-style positions).

### OTG derivative — `otg_v2.yaml` (`kind: otg`, V305/V307…)

Device/host blocks are **byte-for-byte `v2fs`**. Adds `OTG_CR@0x54`,
`OTG_SR@0x58` and ID/VBUS pins. Manual title: USBFS/OTG_FS.

### H4 naming variant — `usbfs_h4.yaml` (`kind: usbfs`)

Same offsets as `otg_v2`; register names carry a `USB_` prefix; device
registers written per-endpoint rather than as arrays. Kept separate so the
PAC matches the H4 headers. **Not a new FS core.**

Driver implication: one USBFS stack with **four backends**
(`v1fs` / `v2fs` / `l1fs` / `x0fs`); `otg` and `usbfs_h4` reuse the `v2fs`
backend plus OTG bring-up.

HAL today: only `l1fs` is implemented (`ch32-hal::usbfs`, `#[cfg(usb_l1fs)]`).
`otg_fs` covers the OTG chips. `v1fs` / `v2fs` / `x0fs` have no device driver
yet.

---

## IP 3 / 4 — USBHS

There are two HS layouts, not one family with versions.

### `usbhs_v3` — IP 3

V305/V307/V317. 16 endpoint slots driven by the `EP_CONFIG` / `EP_TYPE`
bitmaps, and a host block that does not share the V4x7 map. Only `DEV_AD`
matches V4x7 byte for byte; shared register names elsewhere are cosmetic.

### `usbhs_v4x7` / `usbhs_h4` — IP 4

V407 (`v4x7`, instances `USBHS1`/`USBHS2`) and H417 (`h4`). One device-block
layout: the same offsets and endpoint fieldsets, with endpoints 1-7 modelled
as arrays. H4 adds per-endpoint RX/TX FIFO windows (`UEP_RX_FIFO` /
`UEP_TX_FIFO`) at the end of the device block, and a few host fields differ in
width (V407 RM §22.2.1, H417 RM §25.2.1).

HAL today: `ch32-hal::usbhs` uses `src/usbhs/v4x7` for both `usbhs_v4x7` and
`usbhs_h4`; `usbhs_v3` keeps its own `src/usbhs` module.

---

## Out of FS/HS scope

### USBSS (`usbss_h4`)

H417 only. Link / device / host blocks with SS PHY pins. Keep as its own
product line in docs and in any future `crate::usbss`.

### USBPD (`usbpd_*`)

BMC/CC PHY shared by X0, L1, CH641, H4. Documented elsewhere; not a USB
2.0/3.0 controller. Do not fold into USB IP renames.

---

## HAL mapping (ch32-hal)

Pattern to copy from embassy-stm32: one user-facing module, cfg-selected
backend (`embassy_stm32::usb` → `usb.rs` or `otg.rs`).

Target shape for ch32-hal (docs-only until a dedicated PR):

```text
crate::usb              // FS entry (embassy-style)
  ├─ usbd               // IP 1  — kind usbd / v2
  └─ usbfs              // IP 2  — kind usb_*fs, plus otg / usbfs_h4
       backends: v1fs | v2fs | l1fs | x0fs

crate::usbhs            // IP 3/4 — kind usbhs / {v3,v4x7,h4}
crate::usbss            // parked
// usbpd stays separate
```

Current modules vs that target:

| Module | cfg | Actually binds |
| --- | --- | --- |
| `src/usbd` | `usbd` | IP 1 |
| `src/usbfs` | `usb_l1fs` | IP 2 / **l1fs only** |
| `src/otg_fs` | `otg` | IP 2 / v2fs+OTG |
| `src/usbhs` (`mod.rs`) | `usbhs_v3` | IP 3 |
| `src/usbhs/v4x7` | `usbhs_v4x7`, `usbhs_h4` | IP 4 |

The `build.rs` pin gate that drops `kind: usb` rows unless `version == l1fs`
exists because `crate::usbfs` is l1fs-only — not because the other `usb_*fs`
parts are a different peripheral.

---

## Naming collisions already in data

1. **`usbd` means two things in prose.** The kind `usbd` is the classic
   device IP. Many WCH FS YAML files also name their *device block* `USBD`.
   Only `(kind, version)` disambiguates.
2. **Block name `USB` is shared** by `usb_*fs`, `otg_v2`, `usbhs_v3`,
   `usbd_v2`. Newer H4 files already use `USBFS` / `USBHS` / `USBSS`.
3. **`usbd_v2` and `v2fs` coexist** on the same die on some FV2x parts.
4. **`kind: usbfs` (H4) ≠ HAL module `usbfs` (l1fs).** Easy to confuse.

---

## Proposed data changes

**Decision:** do the **soft path** now; **defer hard kind rename**. The
research behind it (RM names, the embassy/stm32-data comparison) stays in
local planning notes, not in this repository.

Goal: make `(kind, version)` match the manual names without a flag-day PAC
break if we can help it.

### Chosen — soft rename via aliases / docs

1. Keep existing `kind`/`version` strings working (PAC stability).
2. Document the four-IP map above as canonical (this file).
3. Prefer **block** renames on older files toward `USBFS` / `USBHS` only when
   something else already forces a PAC break (H4 already uses those names).
4. Keep the canonical alias table here for humans/HAL (it is not a PAC
   feature):

   | Canonical family | Current kinds to treat as one FS stack |
   | --- | --- |
   | `usbd` | `usbd/v2` |
   | `usbfs` | `usb/v1fs`, `usb/v2fs`, `usb/l1fs`, `usb/x0fs`, `otg/v2`, `usbfs/h4` |
   | `usbhs/v3` | `usbhs/v3` |
   | `usbhs/v4x7` | `usbhs/v4x7`, `usbhs/h4` |

### Deferred — kind rename in YAML

Only if we accept regenerating metapac and updating every HAL cfg:

- Collapse OTG into USBFS: move `otg/v2` → `usb/v2fs` (or `usb/v2fs_otg`)
  with OTG registers as an optional fieldset; retarget H4 `usbfs/h4` →
  `usb/h4fs`.
- Leave `usbd` and `usbhs` kind names as they are (they already match the
  manuals).
- Keep `usbhs/h4` on its own version even though its device block matches
  `usbhs/v4x7`. H4 adds FIFO arrays, so folding the two versions would lose
  that distinction; block renames toward `USB_DEVICE` / `USB_HOST` are
  cosmetic only.

Hard rename is **optional hygiene**, not required for correctness or for
porting the missing `v1fs`/`v2fs`/`x0fs` backends. Revisit only when a
shared USBFS stack wants one `pac::usb` path, maintainers accept a metapac
break, and HAL/examples move in the same change.

### HAL follow-up (same refactor effort, separate PR)

1. Introduce `crate::usb` facade (embassy pattern) over `usbd` / current
   `usbfs`(l1fs) / `otg_fs`; keep `usbhs` separate.
2. Teach pin codegen to key off `(kind, version)`, not bare `kind == usb`.
3. Port `v2fs` / `v1fs` / `x0fs` device backends later — **not** Phase 1.

---

## Corrections already applied while researching

(Kept from the earlier audit so they are not rediscovered.)

- `H4_USBFS` base address corrected to `0x40023400` (was FV2x's `0x50000000`).
- CH641 USBPD base corrected to `0x40024000`.
- `usbpd_*` `PORT_CC` modelled as stride-2 registers, not a byte array.
- `usb_l1fs` endpoint DMA/T_LEN offsets and widths fixed to L103 表 20-3.
- `v2fs` / `otg_v2` / `l1fs` host blocks corrected from the V103 layout to
  the FV2x/L103 layout; `TX_LEN@0x3C` restored on `v2fs`/`otg_v2`.
- `usbfs_h4` field widths aligned with `ch32h417.h`.

## Still unverified

- **`x0fs` host block** bit positions (no local CH32X0 RM at audit time).
- Per-endpoint `UEP_T_LEN` width (manuals give 7 bits for most EPs, 10 for
  EP3; fieldsets still declare a uniform 10-bit field).
