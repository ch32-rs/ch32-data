# ch32-data

ch32-data is a project that aims to provide structured, machine-readable data for WCH's 32-bit microcontrollers.

This project is highly inspired by the [stm32-data](https://github.com/embassy-rs/stm32-data) project.
With the following modifications:

- WCH's documentation and tools are not as good as ST's, so the data is not as clean and complete as ST's.
- This project does't have much auto-generated data, most of the chip definitions are manually written.

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

## Minimum supported Rust version(MSRV)

This project is developed with a recent **nightly** version of Rust compiler. And is expected to work with beta versions of Rust.

Feel free to change this if you did some testing with some version of Rust.

## Contributing

TODO. All kinds of contributions are welcome.

## License

This project is licensed under the MIT or Apache-2.0 license, at your option.
