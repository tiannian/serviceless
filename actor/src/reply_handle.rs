use service_channel::oneshot;

use crate::{Error, Message};

pub struct ReplyHandle<M>
where
    M: Message,
{
    sender: Option<oneshot::Sender<M::Result>>,
}

impl<M> ReplyHandle<M>
where
    M: Message,
{
    pub(crate) fn new(sender: Option<oneshot::Sender<M::Result>>) -> Self {
        Self { sender }
    }

    pub fn send(self, value: M::Result) -> std::result::Result<(), Error> {
        if let Some(sender) = self.sender {
            sender.send(value).map_err(|_| Error::ServiceStoped)
        } else {
            Ok(())
        }
    }

    pub fn is_closed(&self) -> bool {
        if let Some(sender) = &self.sender {
            sender.is_canceled()
        } else {
            false
        }
    }
}
