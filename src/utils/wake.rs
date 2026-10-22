use std::sync::LazyLock;
use tokio::sync::Notify;

struct Wake {
    notify: Notify,
}

impl Wake {
    fn new() -> Self {
        Self { notify: Notify::new() }
    }

    fn wake(&self) {
        self.notify.notify_one();
    }

    async fn woken(&self) {
        self.notify.notified().await;
    }
}

static MAIN_LOOP: LazyLock<Wake> = LazyLock::new(Wake::new);

// Asks the main loop to run an iteration now instead of waiting out its tick. Safe to call
// from any thread; several calls before the loop looks collapse into one wake-up.
pub fn wake_main_loop() {
    MAIN_LOOP.wake();
}

pub async fn woken() {
    MAIN_LOOP.woken().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn a_wake_before_the_wait_is_not_lost() {
        let wake = Wake::new();
        wake.wake();
        tokio::time::timeout(Duration::from_millis(200), wake.woken()).await.expect("the earlier wake should still be pending");
    }

    #[tokio::test]
    async fn several_wakes_collapse_into_one() {
        let wake = Wake::new();
        wake.wake();
        wake.wake();
        wake.wake();
        tokio::time::timeout(Duration::from_millis(200), wake.woken()).await.expect("first wait returns at once");
        assert!(tokio::time::timeout(Duration::from_millis(100), wake.woken()).await.is_err(), "no second wake should be pending");
    }

    #[tokio::test]
    async fn a_wake_from_another_thread_ends_a_wait() {
        let wake = std::sync::Arc::new(Wake::new());
        let waker = wake.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            waker.wake();
        });
        tokio::time::timeout(Duration::from_millis(500), wake.woken()).await.expect("the wake from the other thread should arrive");
    }
}
