pub(crate) use std::future::Future;
pub(crate) use async_oneshot::{oneshot as one_channel, Sender as OneTx, Receiver as OneRx};
pub(crate) use async_channel::{unbounded as _req_channel, Sender as _ReqTx, Receiver as _ReqRx};

pub use actor_core::{Context, SyncContext, AsyncContext};

pub type ReqPayload<C> = (<C as Context>::Req, OneTx<<C as Context>::Res>);

pub struct ReqTx<C: Context> {
    // access inner is generally safe
    pub inner: _ReqTx<ReqPayload<C>>,
}

impl<C: Context> ReqTx<C> {
    pub async fn request(&self, req: C::Req) -> Result<C::Res, Option<C::Req>> {
        let (res_tx, res_rx) = one_channel::<C::Res>();
        self.inner.send((req, res_tx)).await.map_err(|payload| Some(payload.0.0))?;
        res_rx.await.map_err(|_| None)
    }
}

impl<C: Context> Clone for ReqTx<C> {
    fn clone(&self) -> Self {
        ReqTx { inner: self.inner.clone() }
    }
}

type CloseFinisher = _ReqTx<()>;
type CloseHandle = _ReqRx<CloseFinisher>;

pub struct CloseHandler {
    tx: _ReqTx<CloseFinisher>,
    rx: _ReqRx<CloseFinisher>,
}

impl CloseHandler {
    pub fn new() -> CloseHandler {
        let (tx, rx) = _req_channel();
        CloseHandler { tx, rx }
    }

    pub fn spawn(&self) -> CloseHandle {
        self.rx.clone()
    }

    pub async fn close(self) {
        let CloseHandler { tx, rx: _ } = self;
        let (tx2, rx2) = _req_channel();
        // send close signal to all actors that are not closed in advance
        if let Err(_) = tx.send(tx2).await {
            // if all actors are closed in advance, return directly
            return;
        }
        // wait all actors that are finished close after received close signal
        let _ = rx2.recv().await.unwrap_err();
    }
}

mod sync;
pub use sync::spawn_sync;
mod r#async;
pub use r#async::spawn_async;
