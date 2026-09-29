extern crate self as uji_native;

pub mod abi;
mod module;

pub use abi::{Held, IntoReply, Json, List, Reply};
#[doc(hidden)]
pub use linkme;
pub use module::{Host, Kernel, Manifest, Script, Symbol, attach, manifest};
pub use uji_macros::{CStruct, native};

pub struct Native {
    pub name: &'static str,
    pub signature: &'static str,
    pub address: fn() -> *const (),
}

pub struct Wrapper {
    pub place: &'static str,
    pub source: &'static str,
}

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
