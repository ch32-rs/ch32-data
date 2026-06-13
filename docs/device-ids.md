# Obtaining chip `device_id` values

Each `data/chips/*.yaml` entry has a `device_id` field per package. This
document explains what it is, how to obtain it for a new chip, and how to
cross-check existing values.

## What is `device_id`?

WCH MCUs expose a 32-bit chip identifier in a memory-mapped read-only word in
the system-memory region. The openwch SDK calls it `IDCODE` and reads it via
`DBGMCU_GetCHIPID()`. The value encodes:

| Bits     | Meaning                                                                |
| -------- | ---------------------------------------------------------------------- |
| `[31:20]`| Family code (e.g. `0x003`, `0x103`, `0x203`, `0x641`).                 |
| `[19:16]`| Package variant (`0`, `1`, `A`, `B`, `D`, ...).                        |
| `[15:8]` | Series / process code (e.g. `0x05`, `0x06`, `0x07`, `0x08`, `0x41`).   |
| `[7:4]`  | Silicon revision (varies; **treat as don't-care** when matching).      |
| `[3:0]`  | Sub-family / extension (`0`, `1`, `2`, `4`, `8`, `C`, `D`, `F`, ...).  |

CH32V103 is the exception: its low 16 bits carry the STM32-compatible
`DBGMCU_IDCODE` device-ID format (`0x4102` for C8T6, `0x410F` for R8T6).

## Where to read it from on-chip

The IDCODE word lives at a different system-memory address per family:

| Family                                    | Address      |
| ----------------------------------------- | ------------ |
| CH32V003                                  | `0x1FFFF7C4` |
| CH32V002 / 004 / 005 / 006 / 007 / M007   | `0x1FFFF704` |
| CH32V103                                  | `0x1FFFF884` |
| CH32V20x (V203 / V208)                    | `0x1FFFF704` |
| CH32V30x (V303 / V305 / V307 / V317)      | `0x1FFFF704` |
| CH32X033 / X035                           | `0x1FFFF704` |
| CH32L103                                  | `0x1FFFF704` |
| CH32H415 / H416 / H417                    | `0x1FFFF704` |
| CH641                                     | `0x1FFFF7C4` |
| CH643                                     | `0x1FFFF704` |

The addresses come from `DBGMCU_GetCHIPID()` in
`openwch/<family>/EVT/EXAM/SRC/Peripheral/src/<family>_dbgmcu.c`.

## How to obtain a `device_id`

Listed in order of authority. **For a chip you do not already have data for,
prefer methods 1–3** over secondary sources.

### 1. Read it from real silicon (most authoritative)

This is the only method that resolves disagreements between upstream sources.

#### a. `wlink status` (recommended if you have a WCH-LinkE)

[wlink](https://github.com/ch32-rs/wlink) prints the raw chip ID it gets from
the probe over USB:

```text
$ wlink status
...
RiscvChip [CHIP_NAME] (ChipID: 0x25004102)
```

The decoded chip name comes from wlink's local table (`src/chips.rs`) — if it
shows `(ChipID: 0xNNNNNNNN)` alone with no name, the table is incomplete and
the raw ID is what you want.

#### b. `minichlink` from [cnlohr/ch32fun](https://github.com/cnlohr/ch32fun)

`minichlink -X` prints the chip's IDCODE alongside other identifiers. Works
with WCH-Link, WCH-LinkE, and the ESP32S2-based programmer.

#### c. Read from on-chip firmware

The most direct method for chips no programmer recognises yet. Add this to a
minimal firmware and observe over UART, SWO, or a debugger:

```c
volatile uint32_t chip_id = *(volatile uint32_t *)0x1FFFF704;  // see address table above
printf("ChipID = 0x%08lX\n", chip_id);
```

The ch32fun `examples_*/template/` projects are a fast way to bring this up.

#### d. OpenOCD direct memory read

With the chip halted under OpenOCD via WCH-LinkE:

```text
> mdw 0x1FFFF704 1     # use the family's address from the table above
0x1ffff704: 25004102
```

This is the rawest method and bypasses every translation layer.

### 2. Runtime `switch (chip_id)` blocks in openwch `*_gpio.c`

When silicon is not on hand, the next-best source is the runtime switch
statements in `openwch/<family>/EVT/EXAM/SRC/Peripheral/src/<family>_gpio.c`.
These are silicon-verified — if the value were wrong, GPIO init would fail on
hardware.

Example from `openwch/ch32v103/EVT/EXAM/SRC/Peripheral/src/ch32v10x_gpio.c`:

```c
chip = *(uint32_t *)0x1FFFF884 & (~0x000000F0);
switch (chip) {
    case 0x25004102:     // CH32V103C8T6
    ...
}
```

The `& (~0x000000F0)` mask tells you which nibbles are revision-stripping
(don't-care). For CH32X035 / CH32X033 / CH643 the mask is `~0x000000F1`, so bit
`[0]` is *also* don't-care for matching — but the actual silicon **does** set
bit `[0]`, and YAML entries record the full silicon value (including the `1`).

Not every family ships a chip-ID switch in `_gpio.c`; when one exists, it is
the strongest non-silicon evidence.

### 3. `GetCHIPID()` comment block in openwch `*_dbgmcu.c`

The function comment in `openwch/<family>/EVT/EXAM/SRC/Peripheral/src/<family>_dbgmcu.c`
documents every documented chip ID. Example:

```c
/*
 * ChipID List-
 *   CH32V103C8T6-0x25004102
 *   CH32V103R8T6-0x2500410F
 */
uint32_t DBGMCU_GetCHIPID(void) {
    return (*(uint32_t *)0x1FFFF884);
}
```

In notation like `0x003005x0`, the `x` means "any hex digit" (the revision
nibble at bits `[7:4]`). Substitute `0` to get a representative full value.

Caveats:

- Comment blocks occasionally lag behind shipped silicon (e.g. they may list
  fewer packages than really exist).
- For chips that *also* have a runtime switch in `_gpio.c`, trust the switch
  over the comment if they ever disagree.
