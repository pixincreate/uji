use std::ffi::{CStr, CString, c_void};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{LazyLock, OnceLock};

use tokio::runtime::Runtime;

use crate::Reply;
use crate::abi::{self, Answer, Owned, Work};
use crate::{CStruct, NATIVES, TYPES, WRAPPERS};

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
pub struct Symbol {
    pub name: *const u8,
    pub signature: *const u8,
    pub address: *const c_void,
}

#[repr(C)]
#[derive(CStruct)]
pub struct Script {
    pub place: *const u8,
    pub source: *const u8,
}

#[repr(C)]
#[derive(CStruct)]
pub struct Manifest {
    pub cdef: *const u8,
    pub natives: *const Symbol,
    pub native_count: usize,
    pub scripts: *const Script,
    pub script_count: usize,
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

fn lasting(value: &str) -> *const u8 {
    let text: &'static CStr = Box::leak(CString::new(value).unwrap_or_default().into_boxed_c_str());
    text.as_ptr().cast()
}

fn cdef() -> String {
    let local = || TYPES.iter().filter(|kind| !kind.name.starts_with(CORE));
    local()
        .map(|kind| format!("typedef struct {0} {0};", kind.name))
        .chain(local().map(|kind| format!("struct {} {{ {} }};", kind.name, kind.fields)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build() -> Shared {
    let natives: &'static [Symbol] = Box::leak(
        NATIVES
            .iter()
            .map(|native| Symbol {
                name: lasting(native.name),
                signature: lasting(native.signature),
                address: (native.address)().cast(),
            })
            .collect(),
    );
    let scripts: &'static [Script] = Box::leak(
        WRAPPERS
            .iter()
            .map(|wrapper| Script {
                place: lasting(wrapper.place),
                source: lasting(wrapper.source),
            })
            .collect(),
    );
    Shared(Manifest {
        cdef: lasting(&cdef()),
        natives: natives.as_ptr(),
        native_count: natives.len(),
        scripts: scripts.as_ptr(),
        script_count: scripts.len(),
    })
}

pub fn manifest() -> *const Manifest {
    std::ptr::from_ref(&MANIFEST.get_or_init(build).0)
}
