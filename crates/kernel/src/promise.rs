use std::cell::RefCell;

use mlua::{MultiValue, Value};
use tokio::sync::oneshot;
use uji_macros::{function, methods};

#[derive(Default)]
pub(crate) struct Promise {
    values: RefCell<Option<Vec<Value>>>,
    waiters: RefCell<Vec<oneshot::Sender<()>>>,
}

impl Promise {
    fn values(&self) -> Option<Vec<Value>> {
        self.values.borrow().clone()
    }
}

#[methods]
impl Promise {
    #[get]
    fn settled(&self) -> bool {
        self.values.borrow().is_some()
    }

    fn resolve(&self, values: MultiValue) -> bool {
        if self.values.borrow().is_some() {
            return false;
        }
        self.values.replace(Some(values.into_vec()));
        self.waiters.take().into_iter().for_each(|waiter| {
            let _ = waiter.send(());
        });
        true
    }

    async fn r#await(&self) -> mlua::Result<MultiValue> {
        if let Some(values) = self.values() {
            return Ok(MultiValue::from_vec(values));
        }
        let (sender, receiver) = oneshot::channel();
        self.waiters.borrow_mut().push(sender);
        receiver
            .await
            .map_err(|_| mlua::Error::runtime("the promise was dropped"))?;
        Ok(MultiValue::from_vec(self.values().unwrap_or_default()))
    }
}

#[function]
fn promise() -> Promise {
    Promise::default()
}
