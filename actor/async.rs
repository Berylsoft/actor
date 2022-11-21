use crate::*;

pub fn spawn_async<C: AsyncContext>(mut ctx: C) -> (impl Future<Output = ()>, ReqTx<C>, CloseHandle) {
    let (close_tx, close_rx) = one_channel();
    let (mut wait_tx, wait_rx) = one_channel();
    let (req_tx, req_rx) = _req_channel::<ReqPayload<C>>();
    let fut = async move {
        use futures_util::FutureExt;
        let mut close_fut = close_rx.fuse();
        let mut recv_fut = req_rx.recv().fuse();
        loop {
            futures_util::select_biased! {
                close = &mut close_fut => {
                    if let Ok(()) = close {
                        ctx.close();
                        wait_tx.send(()).expect("FATAL: close_tx not dropped but wait_rx dropped");
                        break;
                    } else {
                        panic!("close handle dropped before called close()");
                    }
                }
                maybe_req = &mut recv_fut => {
                    if let Ok((req, mut res_tx)) = maybe_req {
                        if let Err(_) = res_tx.send(ctx.exec(req).await) {
                            ctx.close(); // Close and rollback the last request?
                            wait_tx.send(()).expect("FATAL: wait_rx dropped when (all req_tx dropped when sending response)");
                            panic!("FATAL: all req_tx dropped when sending response");
                        }
                    } else {
                        ctx.close();
                        wait_tx.send(()).expect("FATAL: wait_rx dropped when all req_tx dropped");
                        break;
                    }
                }
            }
        }
    };
    (fut, ReqTx { inner: req_tx }, CloseHandle { close_tx, wait_rx })
}
