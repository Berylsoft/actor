use async_oneshot::{oneshot as one_channel, Sender as OneTx};
use async_channel::{unbounded as req_channel, Sender as ReqTx};
use blocking::unblock;
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

#[derive(Clone)]
pub struct Handle<C: Context> {
    req_tx: ReqTx<Message<C>>,
}

pub async fn spawn<C: Context>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let (req_tx, req_rx) = req_channel();
    let mut ctx = unblock(move || C::init(init)).await?;
    unblock(move || {
        if let Ok(Message { req, mut res_tx }) = req_rx.recv_blocking() {
            res_tx.send(match req {
                Request::Req(req) => match ctx.exec(req) {
                    Ok(res) => Response::Res(res),
                    Err(err) => Response::Err(err),
                }
                // active closing
                Request::Close => match ctx.close() {
                    Ok(()) => Response::Closed,
                    Err(err) => Response::Err(err),
                }
            }).expect("FATAL: res_rx dropped before send res");
        } else {
            // passive closing (all request sender dropped)
            ctx.close().expect("FATAL: Error occurred during closing");
        }
    }).detach();
    Ok(Handle { req_tx })
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
