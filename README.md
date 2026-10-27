# ch32-data

[![ch32-metapac on crates.io][badge-metapac-crates]][crates-metapac]
[![ch32-metapac API docs][badge-metapac-docs]][docs-metapac]

[badge-metapac-crates]: https://img.shields.io/crates/v/ch32-metapac.svg?style=for-the-badge&label=ch32-metapac
[crates-metapac]: https://crates.io/crates/ch32-metapac
[badge-metapac-docs]: https://img.shields.io/docsrs/ch32-metapac?style=for-the-badge
[docs-metapac]: https://docs.rs/ch32-metapac

[ch32-data](https://github.com/ch32-rs/ch32-data) is a project that aims to provide structured, machine-readable data for WCH's 32-bit microcontrollers.

Generated peripheral access and chip metadata are published as the [`ch32-metapac`](https://crates.io/crates/ch32-metapac) crate (see [`ch32-metapac-gen/`](ch32-metapac-gen/)).

This project is highly inspired by the [stm32-data](https://github.com/embassy-rs/stm32-data) project.
With the following modifications:

- WCH's documentation and tools are not as good as ST's, so the data is not as clean and complete as ST's.
- This project does't have much auto-generated data, most of the chip definitions are manually written.
- A "include" mechanism is used to reduce the amount of duplicated data, spliting device families into separate files.

All Issues/PRs are accepted at <https://github.com/ch32-rs/ch32-data>, not the nightly metapac repo.

## Data sources

- Datasheets are provided by [wch.cn](https://www.wch.cn/) and [wch-ic.com](https://wch-ic.com/)
- SVD files are provided by [MounRiver Studio](https://www.mounriver.com/)
  - SVD files are post-processed by [ch32-rs](https://github.com/ch32-rs/ch32-rs) project, adding fixes and patches
- Misc info is retrieved from <https://github.com/openwch>

### Families

- CH32V0, Low price (V2A/V2C)
- CH32V1, General purpose (V3A)
- CH32V203/CH32V303, General purpose (V4B)
- CH32V305/CH32V307/CH32V317, High speed interconnect (V4F)
- CH32V407/CH32V467, High speed interconnect, USBHS, Ethernet, PSRAM
- CH32V208, BLE 5.3 (V4C)
- CH32X0, PDUSB (V4C)
- CH32L1, Low power, PDUSB (V4C)
- CH32M103, CH32L103 plus a 3-phase gate driver (V4C)
- CH32H4, Dual-core V3F+V5F, USB 3.0, Gigabit Ethernet, SerDes
- CH643, RGB driver (V4C)
- CH641, USB PD, (V2A)

CH32V407/CH32V467 data covers the VET6, WEU6 and RET6 variants of each family.
Register definitions are checked against CH32V407RM V1.2 and CH32V407DS0 V1.3.
SysTick retains the project's overlapping 64-bit `CNT`/`CMP` and 32-bit low/high-word access views.
The manuals describe a RISC-V3V core; the chip metadata currently uses `qingke-v4f`.
PAC generation and cross-compilation are checked, but runtime compatibility and hardware behavior remain unverified.

Families that not implemented yet(planing to implement using another crate, as they are using different peripherals and features):

- CH57x, BLE 4.2 (V3A)
- CH58x, BLE 5.3 (V4A)
- CH59x, BLE 5.4 (V4C)
- CH569/CH565, USBSS, SerDes (V3A)

For CH58X, you might want to check out my experimental project [ch58x-hal](https://github.com/ch32-rs/ch58x-hal),
which is a HAL for CH58X series chips, with BLE support.

Families that are not yet released(require more info):

- CH564, USBHS, 100M Ethernet (V4J)
- CH645, USB HUB, SerDes (V4C)
- CH32M, Motor control (V2C)

### IP Cores

The CH32 RISC-V series chips are using the Qingke(青稞) IP Core.

Common features:

- Hardware Push Enable (HPE)
- Vector Table Free (VTF)
- 2-wire or single-wire debug interface

Cores:

- V2 (no pmp, no user mode)
  - V2A: rv32ec, +xw
  - V2C: rv32ec, +xw, +Zmmul
- V3 (no pmp)
  - V3A: rv32imac
- V4
  - V4A: rv32imac
  - V4B: rv32imac (no pmp)
  - V4C: rv32imac, +xw
  - V4F: rv32imafc, +xw
  - V4J: rv32imac, I-cache, +xw

## Minimum supported Rust version(MSRV)

This project is developed with a recent **nightly** version of Rust compiler. And is expected to work with beta versions of Rust.

Feel free to change this if you did some testing with some version of Rust.

## Contributing

All kinds of contributions are welcome.

- Bug fixes are welcomed: wrong naming, missing register, typo, translations, etc
- Adding new peripherals requires the following:
  - Use fixed SVD files from [ch32-rs](https://github.com/ch32-rs/ch32-rs) project
  - Use the script `d` to extract the peripheral yaml file from the SVD file (chiptool from embassy project)
  - Editing the yaml file to match the chip's peripherals
  - "transforms" for chiptool are also useful, you can check the [stm32-data](https://github.com/embassy-rs/stm32-data) project for examples
  - Better not writing raw yaml from scratch, copy from existing peripherals and modify it
  - Peripheral files from [stm32-data](https://github.com/embassy-rs/stm32-data) is also acceptable, but make sure to modify it to match the chip's peripherals, and register names
- Editing existing peripherals requires the following:
  - Refer both EVT demo code and datasheet to make sure the data is correct, ref: [openwch](https://github.com/openwch) project
    - Use the newest version of the datasheet! (and Chinese version might be newer than English version)
  - Compare register counts and names first, resolving aliases by offset and function, then compare fields, widths, access and encodings
  - Reuse an existing peripheral version only when the complete register block matches; treat the reference manual as the final authority
  - Preserve intentional project register extensions, such as overlapping SysTick access views
  - Make sure the editing does not breaks [ch32-hal](https://github.com/ch32-rs/ch32-hal)

Run `cargo test -p ch32-data-gen` for register regression tests and `./d gen` to regenerate chip data and PACs.

## License

This project is licensed under the MIT or Apache-2.0 license, at your option.

