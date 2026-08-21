use std::marker::PhantomData;

use crate::{runtime::OneshotSender, Error, Message};

pub struct RuntimedReplyHandle<M, O>
where
    M: Message + Send + 'static,
    M::Result: Send,
    O: OneshotSender<M::Result>,
{
    sender: Option<O>,
    marker: PhantomData<M>,
}

impl<M, O> RuntimedReplyHandle<M, O>
where
    M: Message + Send + 'static,
    M::Result: Send,
    O: OneshotSender<M::Result>,
{
    pub(crate) fn new(sender: Option<O>) -> Self {
        Self {
            sender,
            marker: PhantomData,
        }
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
            sender.is_closed()
        } else {
            false
        }
    }
}
