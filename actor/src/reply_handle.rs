use crate::{Error, Message, OneshotSender, Runtime};

pub struct ReplyHandle<M, R>
where
    M: Message,
    R: Runtime,
{
    sender: Option<R::OneshotSender<M::Result>>,
}

impl<M, R> ReplyHandle<M, R>
where
    M: Message,
    R: Runtime,
{
    pub(crate) fn new(sender: Option<R::OneshotSender<M::Result>>) -> Self {
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
            sender.is_closed()
        } else {
            false
        }
    }
}
