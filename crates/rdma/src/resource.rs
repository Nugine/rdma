use std::sync::{Arc, Weak};

/// # Safety
/// TODO
pub(crate) unsafe trait Resource: Send + Sync + Sized {
    type Owner;

    fn as_owner(&self) -> &Arc<Self::Owner>;

    fn strong_ref(&self) -> Arc<Self::Owner> {
        Arc::clone(self.as_owner())
    }

    fn weak_ref(&self) -> Weak<Self::Owner> {
        Arc::downgrade(self.as_owner())
    }
}
