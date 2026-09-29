use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::task::AbortHandle;
use uji_native::abi::{Owned, Work};

const SHIFT: u32 = 40;
const COUNTER: u64 = (1 << SHIFT) - 1;

type Done = (u64, Owned);

enum Route {
    Attach(u64, mpsc::Sender<Done>),
    Detach(u64),
    Finish(u64, Owned),
}

static RUNS: AtomicU64 = AtomicU64::new(1);
static NEXT: AtomicU64 = AtomicU64::new(1);
static ROUTER: LazyLock<mpsc::Sender<Route>> = LazyLock::new(|| {
    let (router, routes) = mpsc::channel();
    std::thread::spawn(move || route(&routes));
    router
});

fn route(routes: &mpsc::Receiver<Route>) {
    let mut runs: HashMap<u64, mpsc::Sender<Done>> = HashMap::new();
    for route in routes {
        match route {
            Route::Attach(run, sender) => {
                runs.insert(run, sender);
            }
            Route::Detach(run) => {
                runs.remove(&run);
            }
            Route::Finish(token, answer) => {
                if let Some(sender) = runs.get(&(token >> SHIFT)) {
                    drop(sender.send((token, answer)));
                }
            }
        }
    }
}

pub(crate) fn finish(token: u64, answer: Owned) {
    drop(ROUTER.send(Route::Finish(token, answer)));
}

pub(crate) struct Channel {
    run: u64,
    sender: mpsc::Sender<Done>,
    receiver: mpsc::Receiver<Done>,
}

impl Channel {
    pub(crate) fn open() -> Self {
        let run = RUNS.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        drop(ROUTER.send(Route::Attach(run, sender.clone())));
        Self {
            run,
            sender,
            receiver,
        }
    }

    fn token(&self) -> u64 {
        (self.run << SHIFT) | (NEXT.fetch_add(1, Ordering::Relaxed) & COUNTER)
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        drop(ROUTER.send(Route::Detach(self.run)));
    }
}

pub(crate) struct Queue {
    channel: Channel,
    running: HashMap<u64, Option<AbortHandle>>,
    finished: HashMap<u64, Owned>,
}

impl Queue {
    pub(crate) fn new(channel: Channel) -> Self {
        Self {
            channel,
            running: HashMap::new(),
            finished: HashMap::new(),
        }
    }

    pub(crate) fn reserve(&mut self) -> u64 {
        let token = self.channel.token();
        self.running.insert(token, None);
        token
    }

    pub(crate) fn start(&mut self, io: &Handle, work: Work) -> u64 {
        let token = self.channel.token();
        let sender = self.channel.sender.clone();
        let task = io.spawn(async move { drop(sender.send((token, Owned::from(work.await)))) });
        self.running.insert(token, Some(task.abort_handle()));
        token
    }

    fn accept(&mut self, (token, answer): Done) {
        if self.running.remove(&token).is_some() {
            self.finished.insert(token, answer);
        }
    }

    pub(crate) fn poll(&mut self, limit: Option<Duration>) -> u64 {
        while let Ok(done) = self.channel.receiver.try_recv() {
            self.accept(done);
        }
        if self.finished.is_empty() {
            let received = match limit {
                None => self.channel.receiver.recv().ok(),
                Some(limit) => self.channel.receiver.recv_timeout(limit).ok(),
            };
            if let Some(done) = received {
                self.accept(done);
            }
        }
        self.finished.keys().next().copied().unwrap_or_default()
    }

    pub(crate) fn take(&mut self, token: u64) -> Option<Owned> {
        self.finished.remove(&token)
    }

    pub(crate) fn cancel(&mut self, token: u64) {
        if let Some(Some(work)) = self.running.remove(&token) {
            work.abort();
        }
        self.finished.remove(&token);
    }

    pub(crate) fn close(self) -> Channel {
        for work in self.running.into_values().flatten() {
            work.abort();
        }
        self.channel
    }
}
