/// Async runtime abstraction for WASM and native compatibility
///
/// This module provides a unified interface for async operations
/// that works on both native (tokio) and WASM (wasm-bindgen-futures) targets.

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
pub use wasm::*;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::future::Future;
    use std::time::Duration;

    pub async fn sleep(duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    pub fn spawn<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        tokio::spawn(future);
    }

    pub use tokio::sync::Mutex;
    pub use tokio::sync::MutexGuard;
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::future::Future;
    use std::time::Duration;
    use wasm_bindgen_futures::spawn_local;

    pub async fn sleep(duration: Duration) {
        let millis = duration.as_millis() as u32;
        wasm_bindgen_futures::JsFuture::from(
            js_sys::Promise::new(&mut |resolve, _| {
                web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        &resolve,
                        millis as i32,
                    )
                    .unwrap();
            })
        )
        .await
        .unwrap();
    }

    pub fn spawn<F>(future: F)
    where
        F: Future<Output = ()> + 'static,
    {
        spawn_local(future);
    }

    /// WASM-compatible Mutex using std::sync::Mutex
    /// In WASM, we don't need async mutex since there's no threading
    #[derive(Debug)]
    pub struct Mutex<T> {
        inner: std::sync::Mutex<T>,
    }

    impl<T> Mutex<T> {
        pub fn new(inner: T) -> Self {
            Self {
                inner: std::sync::Mutex::new(inner),
            }
        }

        pub async fn lock(&self) -> MutexGuard<'_, T> {
            // In WASM, blocking is okay since there's only one thread
            MutexGuard {
                inner: self.inner.lock().unwrap(),
            }
        }

        pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
            self.inner.try_lock().ok().map(|inner| MutexGuard { inner })
        }
    }

    pub struct MutexGuard<'a, T> {
        inner: std::sync::MutexGuard<'a, T>,
    }

    impl<'a, T> std::ops::Deref for MutexGuard<'a, T> {
        type Target = T;
        fn deref(&self) -> &T {
            &self.inner
        }
    }

    impl<'a, T> std::ops::DerefMut for MutexGuard<'a, T> {
        fn deref_mut(&mut self) -> &mut T {
            &mut self.inner
        }
    }
}
