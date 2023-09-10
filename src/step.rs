use std::error::Error;

use async_trait::async_trait;

#[async_trait]
pub trait AsyncStepService: Send + Sync {
    type Error: StepError + Send + Sync + 'static;

    async fn step(&mut self) -> Result<(), Self::Error>;
}

pub trait StepError: Error {
    fn is_exit(&self) -> bool;
}
