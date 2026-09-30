use std::cell::{Cell, RefCell};

use mlua::{AnyUserData, Lua, MultiValue, Table};
use tokio::sync::oneshot;
use uji_macros::{function, methods};

#[derive(Default)]
pub(crate) struct Promise {
    settled: Cell<bool>,
    waiters: RefCell<Vec<oneshot::Sender<()>>>,
}

impl Promise {
    fn waiter(&self) -> Option<oneshot::Receiver<()>> {
        if self.settled.get() {
            return None;
        }
        let (sender, receiver) = oneshot::channel();
        let mut waiters = self.waiters.borrow_mut();
        waiters.retain(|waiter| !waiter.is_closed());
        waiters.push(sender);
        Some(receiver)
    }

    fn settle(&self) {
        self.settled.set(true);
        self.waiters.take().into_iter().for_each(|waiter| {
            let _ = waiter.send(());
        });
    }
}

fn packed(lua: &Lua, values: MultiValue) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.raw_set("n", values.len())?;
    for (index, value) in values.into_iter().enumerate() {
        table.raw_set(index.saturating_add(1), value)?;
    }
    Ok(table)
}

fn unpacked(promise: &AnyUserData) -> mlua::Result<MultiValue> {
    let table: Table = promise.user_value()?;
    let count: usize = table.raw_get("n")?;
    (1..=count).map(|index| table.raw_get(index)).collect()
}

#[methods]
impl Promise {
    #[get]
    fn settled(&self) -> bool {
        self.settled.get()
    }

    fn resolve(lua: &Lua, promise: &AnyUserData, values: MultiValue) -> mlua::Result<bool> {
        let this = promise.borrow::<Self>()?;
        if this.settled.get() {
            return Ok(false);
        }
        promise.set_user_value(packed(lua, values)?)?;
        this.settle();
        Ok(true)
    }

    async fn r#await(promise: AnyUserData) -> mlua::Result<MultiValue> {
        let waiter = promise.borrow::<Self>()?.waiter();
        if let Some(receiver) = waiter {
            receiver
                .await
                .map_err(|_| mlua::Error::runtime("the promise was dropped"))?;
        }
        unpacked(&promise)
    }
}

#[function]
fn promise() -> Promise {
    Promise::default()
}
