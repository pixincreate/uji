extern crate self as uji_native;

use std::ffi::{CStr, c_char, c_void};

pub mod abi;
mod module;

pub use abi::{Held, IntoReply, Json, List, Reply};
#[doc(hidden)]
pub use linkme;
pub use module::{Host, Kernel, Manifest, attach, declarations, manifest};
pub use uji_macros::{CStruct, native};

#[repr(C)]
#[derive(CStruct)]
pub struct Native {
    name: *const c_char,
    signature: *const c_char,
    address: *const c_void,
}

impl Native {
    pub const fn new(
        name: &'static CStr,
        signature: &'static CStr,
        address: *const c_void,
    ) -> Self {
        Self {
            name: name.as_ptr(),
            signature: signature.as_ptr(),
            address,
        }
    }
}

#[repr(C)]
#[derive(CStruct)]
pub struct Wrapper {
    place: *const c_char,
    source: *const c_char,
}

impl Wrapper {
    pub const fn new(place: &'static CStr, source: &'static CStr) -> Self {
        Self {
            place: place.as_ptr(),
            source: source.as_ptr(),
        }
    }
}

#[allow(unsafe_code)]
unsafe impl Sync for Native {}

#[allow(unsafe_code)]
unsafe impl Sync for Wrapper {}

pub struct Type {
    pub name: &'static str,
    pub fields: &'static str,
}

#[allow(unsafe_code)]
#[linkme::distributed_slice]
pub static NATIVES: [Native];

#[allow(unsafe_code)]
#[linkme::distributed_slice]
pub static WRAPPERS: [Wrapper];

#[allow(unsafe_code)]
#[linkme::distributed_slice]
pub static TYPES: [Type];

#[macro_export]
macro_rules! module {
    () => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn uji_module_init(host: *const $crate::Host) {
            $crate::attach(host);
        }

        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub extern "C" fn uji_module_manifest() -> *const $crate::Manifest {
            $crate::manifest()
        }
    };
}
