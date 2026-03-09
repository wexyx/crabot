
pub trait Constructor<I, O>: Send + Sync + 'static {
    fn create(&self, input: I) -> O;
}

