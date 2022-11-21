use std::{fmt::Debug, future::Future};

pub trait Context {
    type Req: Send + Debug;
    type Res: Send + Debug;
}

pub trait SyncContext: 'static + Sized + Send + Context {
    fn exec(&mut self, req: <Self as Context>::Req) -> <Self as Context>::Res;
    fn close(self) {}
}

pub trait AsyncContext: 'static + Sized + Send + Context {
    type Future: Future<Output = <Self as Context>::Res>;

    fn exec(&mut self, req: <Self as Context>::Req) -> Self::Future;
    fn close(self) {}
}
