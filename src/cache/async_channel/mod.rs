//! Async channel wrappers for non-blocking cache operations

use tokio::sync::{mpsc, oneshot};
use std::future::Future;

/// Async request-response channel
#[allow(dead_code)]
pub struct AsyncRequestChannel<Req, Resp> {
    sender: mpsc::UnboundedSender<(Req, oneshot::Sender<Resp>)>,
}

#[allow(dead_code)]
impl<Req, Resp> AsyncRequestChannel<Req, Resp> {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<(Req, oneshot::Sender<Resp>)>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { sender: tx }, rx)
    }
    
    /// Send request and get future for response (non-blocking!)
    pub async fn request(&self, req: Req) -> Result<Resp, ChannelError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender.send((req, response_tx))
            .map_err(|_| ChannelError::Closed)?;
        response_rx.await.map_err(|_| ChannelError::ResponseLost)
    }
    
    /// Try to send request without blocking (returns future immediately)
    pub fn try_request(&self, req: Req) -> impl Future<Output = Result<Resp, ChannelError>> {
        let (response_tx, response_rx) = oneshot::channel();
        let send_result = self.sender.send((req, response_tx));
        
        async move {
            send_result.map_err(|_| ChannelError::Closed)?;
            response_rx.await.map_err(|_| ChannelError::ResponseLost)
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ChannelError {
    Closed,
    ResponseLost,
}
