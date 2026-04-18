//! Display-sleep prevention for presentation mode.

pub struct PresentationLock {
    _inner: Option<keepawake::KeepAwake>,
}

impl PresentationLock {
    pub fn new() -> Self {
        let inner = match keepawake::Builder::default()
            .display(true)
            .reason("oxlide presentation")
            .app_name("oxlide")
            .app_reverse_domain("io.github.oxlide")
            .create()
        {
            Ok(k) => Some(k),
            Err(e) => {
                eprintln!("warning: failed to acquire display-sleep lock: {e}");
                None
            }
        };
        Self { _inner: inner }
    }
}

impl Default for PresentationLock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_does_not_panic() {
        let _lock = PresentationLock::new();
    }

    #[test]
    fn drop_does_not_panic() {
        let lock = PresentationLock::new();
        drop(lock);
    }
}
