use tokio::task::JoinSet;

pub struct Spawner<T> {
    pub(crate) tasks: JoinSet<T>,
}
