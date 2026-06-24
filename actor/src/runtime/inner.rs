pub trait InnerOp {
    type InnerType;

    fn into_inner(self) -> Self::InnerType;

    fn inner(&self) -> &Self::InnerType;

    fn inner_mut(&mut self) -> &mut Self::InnerType;
}
