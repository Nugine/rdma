#[cfg(test)]
pub(crate) fn require_send_sync<T: Send + Sync>() {}
