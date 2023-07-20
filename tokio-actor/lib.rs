use tokio::sync::oneshot::{channel as one_channel, Sender as OneTx};
use tokio::sync::mpsc::{unbounded_channel as req_channel, UnboundedSender as ReqTx, UnboundedReceiver as ReqRx};
#[inline(always)]
fn unblock<F, R>(func: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::runtime::Handle::current().spawn_blocking(func)
}
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

fn actor<C: Context>(mut ctx: C, mut req_rx: ReqRx<Message<C>>) -> impl FnOnce() {
    move || {
        loop {
            if let Some(Message { req, res_tx }) = req_rx.blocking_recv() {
                match req {
                    Request::Req(req) => {
                        res_tx.send(match ctx.exec(req) {
                            Ok(res) => Response::Res(res),
                            Err(err) => Response::Err(err),
                        }).ok().expect("FATAL: res_rx dropped before send res");
                    },
                    // active closing
                    Request::Close => {
                        res_tx.send(match ctx.close() {
                            Ok(()) => Response::Closed,
                            Err(err) => Response::Err(err),
                        }).ok().expect("FATAL: res_rx dropped before send res");
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

pub fn spawn<C: Context>(ctx: C) -> Handle<C> {
    let (req_tx, req_rx) = req_channel();
    unblock(actor(ctx, req_rx))/* drop is detach */;
    Handle { req_tx }
}

pub async fn spawn_async<C: AsyncInitContext>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let ctx = unblock(move || C::init(init)).await
        .expect("FATAL: tokio runtime error or panic occurred when context init")?;
    Ok(spawn(ctx))
}

pub fn spawn_sync<C: SyncInitContext>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let ctx = C::init(init)?;
    Ok(spawn(ctx))
}

impl<C: Context> Handle<C> {
    #[inline]
    async fn request_raw(&self, req: Request<C>) -> Response<C> {
        let (res_tx, res_rx) = one_channel();
        match self.req_tx.send(Message { req, res_tx }) {
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
