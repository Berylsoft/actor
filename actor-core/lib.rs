pub trait Executor: 'static + Sized + Send {
    type Init;
    type Req: Send + std::fmt::Debug;
    type Res: Send + std::fmt::Debug;

    fn init(init: Self::Init) -> Self;
    fn exec(&mut self, req: Self::Req) -> Self::Res;
    fn close(self) {}
}
