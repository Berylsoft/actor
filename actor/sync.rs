use crate::*;

pub fn spawn_sync<C: SyncContext>(mut ctx: C) -> (impl Future<Output = ()>, ReqTx<C>, CloseHandle) {
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
                        wait_tx.send(()).unwrap();
                        break;
                    } else {
                        unreachable!();
                    }
                }
                maybe_req = &mut recv_fut => {
                    if let Ok((req, mut res_tx)) = maybe_req {
                        res_tx.send(ctx.exec(req)).unwrap();
                    } else {
                        ctx.close();
                        wait_tx.send(()).unwrap();
                        break;
                    }
                }
            }
        }
    };
    (fut, ReqTx { inner: req_tx }, CloseHandle { close_tx, wait_rx })
}
