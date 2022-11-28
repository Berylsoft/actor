use crate::*;

pub fn spawn_sync<C: SyncContext>(mut ctx: C, close_handler: &CloseHandler) -> (impl Future<Output = ()>, ReqTx<C>) {
    let close_handle = close_handler.spawn();
    let (req_tx, req_rx) = _req_channel::<ReqPayload<C>>();
    let fut = async move {
        use futures_util::FutureExt;
        let mut close_fut = close_handle.recv().fuse();
        let mut recv_fut = req_rx.recv().fuse();
        loop {
            futures_util::select_biased! {
                maybe_close_finisher = &mut close_fut => {
                    if let Ok(close_finisher) = maybe_close_finisher {
                        ctx.close();
                        break;
                        // drop close handle & close finisher (finish close after received close signal)
                    } else {
                        panic!("FATAL: close handler dropped before actor");
                    }
                }
                maybe_req = &mut recv_fut => {
                    if let Ok((req, mut res_tx)) = maybe_req {
                        // unreachable: the request sender should keep in the `ReqTx::request()` function context
                        // until received response, so res_rx will not be dropped
                        res_tx.send(ctx.exec(req)).expect("FATAL: unreachable: res_rx dropped when sending response")
                    } else {
                        ctx.close();
                        break;
                        // drop close handle (finish close in advance)
                    }
                }
            }
        }
    };
    (fut, ReqTx { inner: req_tx })
}
