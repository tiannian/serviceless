use service_channel::oneshot;

use crate::{Error, Message};

pub struct ReplyHandle<M>
where
    M: Message,
{
    sender: oneshot::Sender<M::Result>,
}

impl<M> ReplyHandle<M>
where
    M: Message,
{
    pub(crate) fn new(sender: oneshot::Sender<M::Result>) -> Self {
        Self { sender }
    }

    pub fn send(self, value: M::Result) -> std::result::Result<(), Error> {
        self.sender.send(value).map_err(|_| Error::ServiceStoped)
    }

    pub fn is_closed(&self) -> bool {
        self.sender.is_canceled()
    }
}
