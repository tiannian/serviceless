use service_channel::mpsc::UnboundedSender;

use crate::{envelop::EnvelopWithMessage, Message};

/// Address for specific message type
///
/// This address is typed with a specific message type M.
pub struct Address<M>
where
    M: Message,
{
    pub(crate) sender: UnboundedSender<EnvelopWithMessage<M>>,
}
