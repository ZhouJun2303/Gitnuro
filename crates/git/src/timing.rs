//! Correlation carried explicitly into background work; never contains Git arguments.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static STAGES: OnceLock<fn(&str, Duration)> = OnceLock::new();

pub fn set_timing_trace(trace: fn(&str, Duration)) {
    let _ = STAGES.set(trace);
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimingContext {
    pub id: String,
    pub phase: String,
    pub repository: Option<u64>,
}

thread_local! {
    static CONTEXT: RefCell<Option<TimingContext>> = const { RefCell::new(None) };
}

pub fn timing_context() -> Option<TimingContext> {
    CONTEXT.with(|slot| slot.borrow().clone())
}

pub fn timing_repository(repo: &Path) {
    CONTEXT.with(|slot| {
        if let Some(context) = slot.borrow_mut().as_mut() {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            crate::repo_key(repo).hash(&mut hash);
            context.repository = Some(hash.finish());
        }
    });
}

pub struct TimingScope(Option<TimingContext>);

pub fn enter_timing(context: Option<TimingContext>) -> TimingScope {
    TimingScope(CONTEXT.with(|slot| slot.replace(context)))
}

pub struct TimingPhase {
    name: &'static str,
    started: Instant,
    _scope: TimingScope,
}

pub fn timing_phase(name: &'static str) -> TimingPhase {
    let context = timing_context().map(|mut context| {
        context.phase = name.into();
        context
    });
    TimingPhase {
        name,
        started: Instant::now(),
        _scope: enter_timing(context),
    }
}

impl Drop for TimingPhase {
    fn drop(&mut self) {
        if let Some(trace) = STAGES.get() {
            trace(self.name, self.started.elapsed());
        }
    }
}

pub(crate) fn inherit<T>(body: impl FnOnce() -> T) -> impl FnOnce() -> T {
    let context = timing_context();
    move || {
        let _scope = enter_timing(context);
        body()
    }
}

impl Drop for TimingScope {
    fn drop(&mut self) {
        CONTEXT.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_is_restored_after_nested_work_and_panic() {
        let context = TimingContext {
            id: "operation-1".into(),
            phase: "write".into(),
            ..Default::default()
        };
        let _outer = enter_timing(Some(context.clone()));
        let _ = std::panic::catch_unwind(|| {
            let _inner = enter_timing(Some(TimingContext {
                phase: "preview".into(),
                ..context.clone()
            }));
            panic!("interrupted work");
        });
        assert_eq!(timing_context(), Some(context));
    }

    #[test]
    fn worker_explicitly_inherits_context_without_leaking_to_the_next_job() {
        let context = TimingContext {
            id: "operation-2".into(),
            phase: "refs".into(),
            ..Default::default()
        };
        let worker = {
            let _scope = enter_timing(Some(context.clone()));
            inherit(timing_context)
        };
        assert_eq!(timing_context(), None);
        assert_eq!(
            std::thread::spawn(move || {
                let result = worker();
                assert_eq!(timing_context(), None);
                result
            })
            .join()
            .unwrap(),
            Some(context)
        );
    }
}
