use oneshot::{channel as one_channel, Sender as OneTx};
use std::sync::mpsc::{channel as req_channel, Sender as ReqTx};
#[inline(always)]
fn unblock<F, R>(func: F) -> std::thread::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    std::thread::spawn(func)
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

pub fn spawn<C: Context>(init: C::Init) -> Result<Handle<C>, C::Err> {
    let (req_tx, req_rx) = req_channel();
    // straightforward equivalent:
    // let mut ctx = unblock(move || C::init(init)).join()
    //     .expect("FATAL: native thread error or panic occurred when context init")?;
    // but it is actually no need to unblock.
    let mut ctx = C::init(init)?;
    unblock(move || {
        if let Ok(Message { req, res_tx }) = req_rx.recv() {
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
    })/* drop is detach */;
    Ok(Handle { req_tx })
}

impl<C: Context> Handle<C> {
    #[inline]
    fn request_raw(&self, req: Request<C>) -> Response<C> {
        let (res_tx, res_rx) = one_channel();
        match self.req_tx.send(Message { req, res_tx }) {
            Ok(()) => res_rx.recv().expect("FATAL: res_tx dropped before recv res"),
            Err(_) => Response::Closed,
        }
    }

    pub fn request(&self, req: C::Req) -> Result<C::Res, C::Err> {
        match self.request_raw(Request::Req(req)) {
            Response::Res(res) => Ok(res),
            Response::Err(err) => Err(err),
            Response::Closed => Err(ClosedError.into()),
        }
    }

    pub fn wait_close(self) -> Result<(), C::Err> {
        match self.request_raw(Request::Close) {
            Response::Res(_) => panic!("FATAL: res=Res(_) when req=Close"),
            Response::Err(err) => Err(err),
            Response::Closed => Ok(()),
        }
    }
}
