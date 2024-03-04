#![no_std]
#![allow(non_snake_case)]
#![allow(unused)]
#![allow(non_camel_case_types)]
#![doc(html_no_source)]

pub mod common;

#[cfg(feature = "pac")]
include!(env!("CH32_METAPAC_PAC_PATH"));

#[cfg(feature = "metadata")]
pub mod metadata {
    include!("metadata.rs");
    include!(env!("CH32_METAPAC_METADATA_PATH"));
}

pub unsafe trait InterruptNumber: Copy {
    /// Return the interrupt number associated with this variant.
    ///
    /// See trait documentation for safety requirements.
    fn number(self) -> u16;
}
