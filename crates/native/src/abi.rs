#![allow(unsafe_code)]

use std::any::Any;
use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt::Display;
use std::future::Future;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::OnceLock;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use uji_macros::{CStruct, native};

const VALUE: i32 = 0;
const FAILED: i32 = 1;
const END: i32 = 2;
const LATE: i32 = 3;

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

pub trait Fallback {
    fn fallback() -> Self;
}

macro_rules! fallback {
    ($($ty:ty => $value:expr),* $(,)?) => {
        $(impl Fallback for $ty {
            fn fallback() -> Self {
                $value
            }
        })*
    };
}

fallback! {
    () => (),
    bool => false,
    i8 => -1,
    i16 => -1,
    i32 => -1,
    i64 => -1,
    u8 => 0,
    u16 => 0,
    u32 => 0,
    u64 => 0,
    usize => 0,
    f32 => 0.0,
    f64 => 0.0,
}

impl<T> Fallback for *mut T {
    fn fallback() -> Self {
        std::ptr::null_mut()
    }
}

impl<T> Fallback for *const T {
    fn fallback() -> Self {
        std::ptr::null()
    }
}

pub fn guard<R: Fallback>(run: impl FnOnce() -> R) -> R {
    catch_unwind(AssertUnwindSafe(run)).unwrap_or_else(|panic| {
        let message = panic
            .downcast_ref::<&str>()
            .map(ToString::to_string)
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "a native function panicked".to_string());
        fail(&message);
        R::fallback()
    })
}

pub fn fail(message: &impl ToString) {
    LAST_ERROR.with(|last| *last.borrow_mut() = message.to_string());
}

pub fn last_error() -> String {
    LAST_ERROR.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

pub(crate) fn bytes<'a>(data: *const u8, length: usize) -> &'a [u8] {
    if data.is_null() || length == 0 {
        return &[];
    }
    unsafe { std::slice::from_raw_parts(data, length) }
}

pub(crate) fn reference<'a, T>(pointer: *const T) -> Option<&'a T> {
    if pointer.is_null() {
        return None;
    }
    Some(unsafe { &*pointer })
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct In(*const u8);

impl In {
    pub fn bytes<'a>(self, length: usize) -> &'a [u8] {
        bytes(self.0, length)
    }

    pub fn text<'a>(self, length: usize) -> Cow<'a, str> {
        String::from_utf8_lossy(self.bytes(length))
    }

    pub fn given<'a>(self, length: usize) -> Option<Cow<'a, str>> {
        (!self.0.is_null()).then(|| self.text(length))
    }

    pub fn json<T: DeserializeOwned>(self, length: usize) -> Option<Json<T>> {
        serde_json::from_slice(self.bytes(length))
            .map(Json)
            .map_err(|err| fail(&err))
            .ok()
    }
}

#[repr(transparent)]
pub struct Out<T>(*mut T);

impl<T> Out<T> {
    pub fn write(self, value: T) {
        if !self.0.is_null() {
            unsafe { self.0.write(value) };
        }
    }
}

type Boxed = Box<dyn Any>;

pub fn handle(value: Boxed) -> *mut c_void {
    Box::into_raw(Box::new(value)).cast()
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Handle(*mut c_void);

impl Handle {
    pub fn borrow<'a, T: Any>(self) -> Option<&'a T> {
        let Some(held) = reference(self.0.cast::<Boxed>()) else {
            fail(&"this object has been closed");
            return None;
        };
        let found = held.downcast_ref::<T>();
        if found.is_none() {
            fail(&"a native call got a handle of the wrong kind");
        }
        found
    }
}

#[native]
fn uji_release(handle: *mut c_void) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle.cast::<Boxed>()) });
    }
}

pub struct Json<T>(pub T);

#[derive(Default)]
pub struct List<T>(pub Vec<T>);

#[derive(Deserialize)]
#[serde(untagged)]
enum Listed<T> {
    Items(Vec<T>),
    Empty {},
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for List<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Listed::deserialize(deserializer)? {
            Listed::Items(items) => Self(items),
            Listed::Empty {} => Self(Vec::new()),
        })
    }
}

pub struct Held<T, F = ()>(pub T, pub F);

impl<T> Held<T> {
    pub fn new(value: T) -> Self {
        Self(value, ())
    }
}

#[repr(C)]
#[derive(CStruct)]
pub struct Answer {
    pub status: i32,
    pub data: *const u8,
    pub length: usize,
    pub handle: *mut c_void,
    pub free: extern "C" fn(*mut Answer),
    pub release: extern "C" fn(*mut c_void),
}

#[repr(C)]
struct Sent {
    answer: Answer,
    data: Vec<u8>,
}

extern "C" fn free(answer: *mut Answer) {
    if !answer.is_null() {
        let sent = unsafe { Box::from_raw(answer.cast::<Sent>()) };
        uji_release(sent.answer.handle);
    }
}

pub struct Reply {
    status: i32,
    data: Vec<u8>,
    held: Option<Box<dyn Any + Send>>,
}

impl Reply {
    fn new(status: i32, data: Vec<u8>) -> Self {
        Self {
            status,
            data,
            held: None,
        }
    }

    pub fn status(status: i32, data: &[u8]) -> Self {
        Self::new(status, data.to_vec())
    }

    pub fn value(data: impl Into<Vec<u8>>) -> Self {
        Self::new(VALUE, data.into())
    }

    pub fn failed(message: &impl ToString) -> Self {
        Self::new(FAILED, message.to_string().into_bytes())
    }

    pub fn end() -> Self {
        Self::new(END, Vec::new())
    }

    pub fn late() -> Self {
        Self::new(LATE, Vec::new())
    }

    pub fn into_raw(self) -> *mut Answer {
        let mut sent = Box::new(Sent {
            answer: Answer {
                status: self.status,
                data: std::ptr::null(),
                length: 0,
                handle: self.held.map_or(std::ptr::null_mut(), |held| handle(held)),
                free,
                release: uji_release,
            },
            data: self.data,
        });
        sent.answer.data = sent.data.as_ptr();
        sent.answer.length = sent.data.len();
        Box::into_raw(sent).cast()
    }
}

pub struct Owned(*mut Answer);

unsafe impl Send for Owned {}

impl Owned {
    pub(crate) fn adopt(answer: *mut Answer) -> Option<Self> {
        (!answer.is_null()).then_some(Self(answer))
    }

    pub fn into_raw(self) -> *mut Answer {
        ManuallyDrop::new(self).0
    }
}

impl From<Reply> for Owned {
    fn from(reply: Reply) -> Self {
        Self(reply.into_raw())
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        if let Some(answer) = reference(self.0) {
            (answer.free)(self.0);
        }
    }
}

#[native]
fn uji_error() -> *mut Answer {
    Reply::value(last_error()).into_raw()
}

pub fn answered(answer: *mut Answer) -> *mut Answer {
    if answer.is_null() {
        return Reply::failed(&last_error()).into_raw();
    }
    answer
}

pub type Work = Pin<Box<dyn Future<Output = Reply> + Send>>;

static SPAWN: OnceLock<fn(Work) -> u64> = OnceLock::new();

pub fn install(spawn: fn(Work) -> u64) {
    let _ = SPAWN.set(spawn);
}

pub fn start(work: impl Future<Output = Reply> + Send + 'static) -> u64 {
    let Some(spawn) = SPAWN.get() else {
        fail(&"this library was loaded without a uji host");
        return 0;
    };
    spawn(Box::pin(work))
}

pub fn issued(token: u64) -> u64 {
    if token != 0 {
        return token;
    }
    let failure = Reply::failed(&last_error());
    start(async move { failure })
}

pub trait IntoReply {
    fn into_reply(self) -> Reply;
}

impl IntoReply for Reply {
    fn into_reply(self) -> Reply {
        self
    }
}

impl IntoReply for () {
    fn into_reply(self) -> Reply {
        Reply::value(Vec::new())
    }
}

impl IntoReply for Vec<u8> {
    fn into_reply(self) -> Reply {
        Reply::value(self)
    }
}

macro_rules! numbers {
    ($($ty:ty),*) => {
        $(impl IntoReply for $ty {
            fn into_reply(self) -> Reply {
                Reply::value(self.to_string())
            }
        })*
    };
}

numbers!(usize, u64, i64);

impl IntoReply for String {
    fn into_reply(self) -> Reply {
        Reply::value(self)
    }
}

impl<T: Serialize> IntoReply for Json<T> {
    fn into_reply(self) -> Reply {
        serde_json::to_vec(&self.0).map_or_else(|err| Reply::failed(&err), Reply::value)
    }
}

impl<T: Any + Send, F: Serialize> IntoReply for Held<T, F> {
    fn into_reply(self) -> Reply {
        Reply {
            held: Some(Box::new(self.0)),
            ..Json(self.1).into_reply()
        }
    }
}

impl<A: Serialize, B: Serialize> IntoReply for (A, B) {
    fn into_reply(self) -> Reply {
        Json(self).into_reply()
    }
}

impl<T: IntoReply> IntoReply for Option<T> {
    fn into_reply(self) -> Reply {
        self.map_or_else(Reply::end, IntoReply::into_reply)
    }
}

impl<T: IntoReply, E: Display> IntoReply for Result<T, E> {
    fn into_reply(self) -> Reply {
        self.map_or_else(|err| Reply::failed(&err), IntoReply::into_reply)
    }
}
