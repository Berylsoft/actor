use std::fmt::Debug;

pub trait Executor: 'static + Sized + Send {
    type Req: Send + Debug;
    type Res: Send + Debug;

    fn exec(&mut self, req: Self::Req) -> Self::Res;
    fn close(self) {}
}
