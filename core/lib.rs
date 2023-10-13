pub trait Context: Sized + Send + 'static {
    type Req: Sync + Send + 'static;
    type Res: Sync + Send + 'static;
    type Err: Sync + Send + 'static + From<ClosedError> + core::fmt::Debug;

    fn exec(&mut self, req: Self::Req) -> Result<Self::Res, Self::Err>;
    fn close(self) -> Result<(), Self::Err>;
}

pub trait AsyncInitContext: Context {
    type Init: Send + 'static;

    fn init(init: Self::Init) -> Result<Self, Self::Err>;
}

pub trait SyncInitContext: Context {
    type Init;

    fn init(init: Self::Init) -> Result<Self, Self::Err>;
}

pub struct ClosedError;

impl std::fmt::Debug for ClosedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClosedError")
    }
}

impl std::fmt::Display for ClosedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClosedError")
    }
}

impl std::error::Error for ClosedError {}
