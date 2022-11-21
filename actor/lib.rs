pub(crate) use std::future::Future;
pub(crate) use async_oneshot::{oneshot as one_channel, Sender as OneTx, Receiver as OneRx};
pub(crate) use async_channel::{unbounded as _req_channel, Sender as _ReqTx};

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

pub struct CloseHandle {
    close_tx: OneTx<()>,
    wait_rx: OneRx<()>,
}

impl CloseHandle {
    pub async fn close(self) -> Option<()> {
        let CloseHandle { mut close_tx, wait_rx } = self;
        if let Ok(()) = close_tx.send(()) {
            wait_rx.await.ok()
        } else {
            // close_rx dropped i.e. ctx already closed
            Some(())
        }
    }
}

mod sync;
pub use sync::spawn_sync;
mod r#async;
pub use r#async::spawn_async;
