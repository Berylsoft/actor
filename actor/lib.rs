#[cfg(not(any(feature = "sync", feature = "async")))]
compile_error!("choose sync or async or both");

use async_oneshot::{oneshot as one_channel, Sender as OneTx};
use async_channel::{unbounded as req_channel, Sender as ReqTx, Receiver as ReqRx};
#[cfg(feature = "sync")]
use blocking::unblock;
#[cfg(feature = "async")]
use async_global_executor::spawn;
use actor_core::*;

enum Request<C: Context> {
    Req(C::Req),
    Close,
}

enum Response<C: Context> {
    Res(C::Res),
    Err(C::Err),
    Closed,
}

struct Message<C: Context> {
    req: Request<C>,
    res_tx: OneTx<Response<C>>,
}

pub struct Handle<C: Context> {
    req_tx: ReqTx<Message<C>>,
}

impl<C: Context> Clone for Handle<C> {
    fn clone(&self) -> Self {
        Self { req_tx: self.req_tx.clone() }
    }
}

#[cfg(feature = "sync")]
fn sync_actor<C: SyncContext>(mut ctx: C, req_rx: ReqRx<Message<C>>) -> impl FnOnce() {
    move || {
        loop {
            if let Ok(Message { req, mut res_tx }) = req_rx.recv_blocking() {
                match req {
                    Request::Req(req) => {
                        res_tx.send(match ctx.exec(req) {
                            Ok(res) => Response::Res(res),
                            Err(err) => Response::Err(err),
                        }).expect("FATAL: res_rx dropped before send res");
                    },
                    // active closing
                    Request::Close => {
                        res_tx.send(match ctx.close() {
                            Ok(()) => Response::Closed,
                            Err(err) => Response::Err(err),
                        }).expect("FATAL: res_rx dropped before send res");
                        break;
                    } 
                }
            } else {
                // passive closing (all request sender dropped)
                ctx.close().expect("FATAL: Error occurred during closing");
                break;
            }
        }
    }
}

#[cfg(feature = "async")]
fn async_actor<C: AsyncContext>(mut ctx: C, req_rx: ReqRx<Message<C>>) -> impl core::future::Future<Output = ()> {
    async move {
        loop {
            if let Ok(Message { req, mut res_tx }) = req_rx.recv().await {
                match req {
                    Request::Req(req) => {
                        res_tx.send(match ctx.exec(req).await {
                            Ok(res) => Response::Res(res),
                            Err(err) => Response::Err(err),
                        }).expect("FATAL: res_rx dropped before send res");
                    },
                    // active closing
                    Request::Close => {
                        res_tx.send(match ctx.close().await {
                            Ok(()) => Response::Closed,
                            Err(err) => Response::Err(err),
                        }).expect("FATAL: res_rx dropped before send res");
                        break;
                    } 
                }
            } else {
                // passive closing (all request sender dropped)
                ctx.close().await.expect("FATAL: Error occurred during closing");
                break;
            }
        }
    }
}

#[cfg(feature = "sync")]
pub fn spawn_sync<C: SyncContext>(ctx: C) -> Handle<C> {
    let (req_tx, req_rx) = req_channel();
    unblock(sync_actor(ctx, req_rx)).detach();
    Handle { req_tx }
}

#[cfg(feature = "async")]
pub fn spawn_async<C: AsyncContext>(ctx: C) -> Handle<C> {
    let (req_tx, req_rx) = req_channel();
    spawn(async_actor(ctx, req_rx)).detach();
    Handle { req_tx }
}

#[cfg(feature = "sync")]
pub async fn create_async_sync<C: AsyncInitContext + SyncContext>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let ctx = C::init(init).await?;
    Ok(spawn_sync(ctx))
}

#[cfg(feature = "sync")]
pub async fn create_sync_sync<C: SyncInitContext + SyncContext>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let ctx = unblock(move || C::init(init)).await?;
    Ok(spawn_sync(ctx))
}

#[cfg(feature = "sync")]
pub fn create_simple_sync<C: SimpleInitContext + SyncContext>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let ctx = C::init(init)?;
    Ok(spawn_sync(ctx))
}

#[cfg(feature = "async")]
pub async fn create_async_async<C: AsyncInitContext + AsyncContext>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let ctx = C::init(init).await?;
    Ok(spawn_async(ctx))
}

#[cfg(feature = "async")]
pub async fn create_sync_async<C: SyncInitContext + AsyncContext>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let ctx = unblock(move || C::init(init)).await?;
    Ok(spawn_async(ctx))
}

#[cfg(feature = "async")]
pub fn create_simple_async<C: SimpleInitContext + AsyncContext>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let ctx = C::init(init)?;
    Ok(spawn_async(ctx))
}

impl<C: Context> Handle<C> {
    #[inline]
    async fn request_raw(&self, req: Request<C>) -> Response<C> {
        let (res_tx, res_rx) = one_channel();
        match self.req_tx.send(Message { req, res_tx }).await {
            Ok(()) => res_rx.await.expect("FATAL: res_tx dropped before recv res"),
            Err(_) => Response::Closed,
        }
    }

    pub async fn request(&self, req: C::Req) -> Result<C::Res, C::Err> {
        match self.request_raw(Request::Req(req)).await {
            Response::Res(res) => Ok(res),
            Response::Err(err) => Err(err),
            Response::Closed => Err(ClosedError.into()),
        }
    }

    pub async fn wait_close(self) -> Result<(), C::Err> {
        match self.request_raw(Request::Close).await {
            Response::Res(_) => panic!("FATAL: res=Res(_) when req=Close"),
            Response::Err(err) => Err(err),
            Response::Closed => Ok(()),
        }
    }
}
