use std::ffi::{CString, c_char};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{LazyLock, OnceLock};

use tokio::runtime::Runtime;

use crate::Reply;
use crate::abi::{self, Answer, Owned, Work};
use crate::{CStruct, NATIVES, Native, TYPES, Type, WRAPPERS, Wrapper};

const CORE: &str = "uji_";

pub trait Kernel {
    fn reserve() -> u64;
    fn finish(token: u64, answer: Owned);
}

#[repr(C)]
#[derive(CStruct)]
pub struct Host {
    reserve: extern "C" fn() -> u64,
    finish: extern "C" fn(u64, *mut Answer),
    complete: extern "C" fn(u64, i32, *const u8, usize),
}

impl Host {
    pub const fn of<K: Kernel>() -> Self {
        Self {
            reserve: reserve::<K>,
            finish: finish::<K>,
            complete: complete::<K>,
        }
    }
}

extern "C" fn reserve<K: Kernel>() -> u64 {
    abi::guard(K::reserve)
}

extern "C" fn finish<K: Kernel>(token: u64, answer: *mut Answer) {
    abi::guard(|| {
        if let Some(answer) = Owned::adopt(answer) {
            K::finish(token, answer);
        }
    });
}

extern "C" fn complete<K: Kernel>(token: u64, status: i32, data: *const u8, length: usize) {
    abi::guard(|| {
        let reply = Reply::status(status, abi::bytes(data, length));
        K::finish(token, Owned::from(reply));
    });
}

#[repr(C)]
#[derive(CStruct)]
pub struct Manifest {
    pub cdef: *const c_char,
    pub natives: *const Native,
    pub native_count: usize,
    pub wrappers: *const Wrapper,
    pub wrapper_count: usize,
}

struct Shared(Manifest);

#[allow(unsafe_code)]
unsafe impl Send for Shared {}

#[allow(unsafe_code)]
unsafe impl Sync for Shared {}

static HOST: AtomicPtr<Host> = AtomicPtr::new(std::ptr::null_mut());
static RUNTIME: LazyLock<Option<Runtime>> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .ok()
});
static LOCAL: OnceLock<CString> = OnceLock::new();
static MANIFEST: OnceLock<Shared> = OnceLock::new();

fn spawn(work: Work) -> u64 {
    let Some(host) = abi::reference(HOST.load(Ordering::Acquire).cast_const()) else {
        abi::fail(&"this library was loaded without a uji host");
        return 0;
    };
    let Some(runtime) = RUNTIME.as_ref() else {
        abi::fail(&"this library could not start its runtime");
        return 0;
    };
    let token = (host.reserve)();
    if token == 0 {
        abi::fail(&"the uji kernel is not running");
        return 0;
    }
    let finish = host.finish;
    drop(runtime.spawn(async move { finish(token, Owned::from(work.await).into_raw()) }));
    token
}

pub fn attach(host: *const Host) {
    HOST.store(host.cast_mut(), Ordering::Release);
    abi::install(spawn);
}

fn declare<'a>(types: impl Iterator<Item = &'a Type> + Clone) -> String {
    types
        .clone()
        .map(|kind| format!("typedef struct {0} {0};", kind.name))
        .chain(types.map(|kind| format!("struct {} {{ {} }};", kind.name, kind.fields)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn declarations() -> String {
    declare(TYPES.iter())
}

pub fn manifest() -> *const Manifest {
    let shared = MANIFEST.get_or_init(|| {
        let local = LOCAL.get_or_init(|| {
            let types = TYPES.iter().filter(|kind| !kind.name.starts_with(CORE));
            CString::new(declare(types)).unwrap_or_default()
        });
        Shared(Manifest {
            cdef: local.as_ptr(),
            natives: NATIVES.as_ptr(),
            native_count: NATIVES.len(),
            wrappers: WRAPPERS.as_ptr(),
            wrapper_count: WRAPPERS.len(),
        })
    });
    std::ptr::from_ref(&shared.0)
}
