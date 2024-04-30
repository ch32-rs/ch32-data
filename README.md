# ch32-data

[ch32-data](https://github.com/ch32-rs/ch32-data) is a project that aims to provide structured, machine-readable data for WCH's 32-bit microcontrollers.

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

- CH32V
- CH32X
- CH32L
- CH643
- CH645
- CH641

Families that not implemented yet(planing to implement using another crate, as they are using different peripherals and features):

- CH57x/CH58x/CH59x
- CH569/CH565

For CH58X, you might want to check out my experimental project [ch58x-hal](https://github.com/ch32-rs/ch58x-hal),
which is a HAL for CH58X series chips, with BLE support.

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
  - Make sure the editing does not breaks [ch32-hal](https://github.com/ch32-rs/ch32-hal)

## License

This project is licensed under the MIT or Apache-2.0 license, at your option.
