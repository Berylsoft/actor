pub trait Context: Sized + Send + 'static {
    type Init: Send + 'static;
    type Req: Sync + Send + 'static;
    type Res: Sync + Send + 'static;
    type Err: Sync + Send + 'static + From<ClosedError> + core::fmt::Debug;

    fn init(init: Self::Init) -> Result<Self, Self::Err>;
    fn exec(&mut self, req: Self::Req) -> Result<Self::Res, Self::Err>;
    fn close(self) -> Result<(), Self::Err>;
}

pub struct ClosedError;
