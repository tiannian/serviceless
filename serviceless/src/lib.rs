mod __tokio_runtime {
    use futures_core::Stream;
    use serviceless_actor::{Context, Envelope, Metadata, RuntimedService, RuntimedServiceAddress};

    use async_trait::async_trait;

    #[async_trait]
    pub trait Service: Send + Sized + 'static {
        type Stream: Stream<Item = Envelope<Self>> + Unpin + Send;

        type Error: Send;

        fn metadata(&self) -> Metadata<'_>;

        /// Start a service with the given context
        ///
        /// Returns the address and a future that should be spawned to run the service.
        /// The caller is responsible for spawning the returned future using their async runtime.
        fn start_by_context(
            self,
            ctx: Context<Self>,
        ) -> (
            RuntimedServiceAddress<Self>,
            impl Future<Output = Result<(), Self::Error>> + Send,
        ) {
            ctx.run(self)
        }

        /// Hook for service started
        async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
            Ok(())
        }

        /// Hook for service stopped
        async fn stopped(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[async_trait]
    impl<T> RuntimedService for T
    where
        T: Service,
    {
        type Stream = T::Stream;

        type Error = T::Error;

        type Runtime = serviceless_tokio::TokioRuntime;

        fn metadata(&self) -> Metadata<'_> {
            self.metadata()
        }

        fn start_by_context(
            self,
            ctx: Context<Self>,
        ) -> (
            RuntimedServiceAddress<Self>,
            impl Future<Output = Result<(), Self::Error>> + Send,
        ) {
            self.start_by_context(ctx)
        }

        async fn started(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
            self.started(_ctx).await
        }

        async fn stopped(&mut self, _ctx: &mut Context<Self>) -> Result<(), Self::Error> {
            self.stopped(_ctx).await
        }
    }
}
pub use __tokio_runtime::*;
