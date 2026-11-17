//! A client node that runs one async function once it starts.

use citadel_sdk::prelude::*;
use futures::lock::Mutex;

pub struct ClientKernel<F> {
    f: Mutex<Option<F>>,
    remote: Option<NodeRemote<StackedRatchet>>,
}

impl<F> std::fmt::Debug for ClientKernel<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientKernel").finish_non_exhaustive()
    }
}

impl<F> ClientKernel<F> {
    pub fn new(f: F) -> Self {
        Self {
            f: Mutex::new(Some(f)),
            remote: None,
        }
    }
}

#[async_trait]
impl<F, Fut> NetKernel<StackedRatchet> for ClientKernel<F>
where
    F: FnOnce(NodeRemote<StackedRatchet>) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<(), NetworkError>> + Send,
{
    fn load_remote(&mut self, remote: NodeRemote<StackedRatchet>) -> Result<(), NetworkError> {
        self.remote = Some(remote);
        Ok(())
    }
    async fn on_start(&self) -> Result<(), NetworkError> {
        let f = self
            .f
            .lock()
            .await
            .take()
            .ok_or_else(|| NetworkError::msg("started twice"))?;
        let remote = self
            .remote
            .clone()
            .ok_or_else(|| NetworkError::msg("no remote"))?;
        f(remote).await
    }
    async fn on_node_event_received(
        &self,
        _: NodeResult<StackedRatchet>,
    ) -> Result<(), NetworkError> {
        Ok(())
    }
    async fn on_stop(&mut self) -> Result<(), NetworkError> {
        Ok(())
    }
}
