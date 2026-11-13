//! Share only overlapping reads. Finished results and failures are not cached.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};

type Erased = dyn Any + Send + Sync;
type Flights = HashMap<(TypeId, String), Weak<Erased>>;

struct Flight<T> {
    value: Mutex<Option<Result<T, String>>>,
    ready: Condvar,
}

pub fn shared<T: Clone + Send + Sync + 'static>(
    key: String,
    read: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    static FLIGHTS: OnceLock<Mutex<Flights>> = OnceLock::new();
    let (flight, leader) = {
        let mut flights = FLIGHTS
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        flights.retain(|_, value| value.strong_count() != 0);
        let key = (TypeId::of::<T>(), key);
        if let Some(flight) = flights
            .get(&key)
            .and_then(Weak::upgrade)
            .and_then(|value| value.downcast::<Flight<T>>().ok())
        {
            (flight, false)
        } else {
            let flight = Arc::new(Flight {
                value: Mutex::new(None),
                ready: Condvar::new(),
            });
            let erased: Arc<Erased> = flight.clone();
            flights.insert(key, Arc::downgrade(&erased));
            (flight, true)
        }
    };
    if leader {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(read))
            .unwrap_or_else(|_| Err("repository read stopped unexpectedly".into()));
        *flight
            .value
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(result);
        flight.ready.notify_all();
    }
    let mut value = flight
        .value
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    while value.is_none() {
        value = flight
            .ready
            .wait(value)
            .unwrap_or_else(|error| error.into_inner());
    }
    value.as_ref().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn concurrent_reads_share_work_but_the_next_read_runs_again() {
        let calls = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            let (started, ready) = std::sync::mpsc::channel();
            let calls = &calls;
            let first = scope.spawn(move || {
                shared("read-sharing-test".into(), || {
                    calls.fetch_add(1, Ordering::SeqCst);
                    started.send(()).unwrap();
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    Ok(7u32)
                })
            });
            ready.recv().unwrap();
            let second = shared("read-sharing-test".into(), || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(9u32)
            });
            assert_eq!(second.unwrap(), first.join().unwrap().unwrap());
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            shared("read-sharing-test".into(), || Ok(11u32)).unwrap(),
            11
        );
    }

    #[test]
    fn interrupted_reader_unblocks_waiters_and_does_not_cache_failure() {
        assert!(shared::<u32>("failed-read-test".into(), || panic!("read interrupted")).is_err());
        assert_eq!(shared("failed-read-test".into(), || Ok(13u32)).unwrap(), 13);
    }
}
