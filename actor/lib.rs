pub use request_channel;

use tokio::sync::oneshot;
use request_channel::unbounded::{ReqTx, channel};
use actor_core::Executor;

pub struct CloseHandle {
    tx: Option<oneshot::Sender<()>>,
    rx: oneshot::Receiver<()>,
}

impl CloseHandle {
    pub fn close(&mut self) -> Option<()> {
        self.tx.take().unwrap().send(()).ok()
    }

    pub async fn wait(self) -> Option<()> {
        self.rx.await.ok()
    }
}

pub fn spawn<E: Executor>(init: E::Init) -> (ReqTx<E::Req, E::Res>, CloseHandle) {
    let (tx, mut wait) = oneshot::channel();
    let (finish, rx) = oneshot::channel();
    let (req_tx, mut req_rx) = channel::<E::Req, E::Res>();
    let mut ctx = E::init(init);
    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                Ok(()) = &mut wait => {
                    ctx.close();
                    finish.send(()).unwrap();
                    break;
                }
                maybe_req = req_rx.recv() => {
                    if let Some((req, res_tx)) = maybe_req {
                        res_tx.send(ctx.exec(req)).unwrap();
                    } else {
                        ctx.close();
                        finish.send(()).unwrap();
                        break;
                    }
                }
            }
        }    
    });
    (req_tx, CloseHandle { tx: Some(tx), rx })
}
