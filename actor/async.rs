use crate::*;

pub fn spawn_async<C: AsyncContext>(mut ctx: C) -> (impl Future<Output = ()>, ReqTx<C>, CloseHandle) {
    let (tx, wait) = one_channel();
    let (mut finish, rx) = one_channel();
    let (req_tx, req_rx) = _req_channel::<ReqPayload<C>>();
    let fut = async move {
        use futures_util::FutureExt;
        let mut wait_fut = wait.fuse();
        let mut recv_fut = req_rx.recv().fuse();
        loop {
            futures_util::select_biased! {
                close = &mut wait_fut => {
                    if let Ok(()) = close {
                        ctx.close();
                        finish.send(()).unwrap();
                        break;
                    } else {
                        unreachable!();
                    }
                }
                maybe_req = &mut recv_fut => {
                    if let Ok((req, mut res_tx)) = maybe_req {
                        res_tx.send(ctx.exec(req).await).unwrap();
                    } else {
                        ctx.close();
                        finish.send(()).unwrap();
                        break;
                    }
                }
            }
        }
    };
    (fut, ReqTx { inner: req_tx }, CloseHandle { tx: Some(tx), rx })
}
