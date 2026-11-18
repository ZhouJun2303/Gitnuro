use awegit_git::flow::Event;

/// Equal command phases, including preparation and final status verification.
/// Conditional save/restore phases finish when skipped; elapsed time never advances progress.
pub(super) struct Progress {
    stages: Vec<&'static str>,
    done: Vec<bool>,
    active: usize,
    pub phase: &'static str,
    pub percent: u8,
}

impl Progress {
    pub fn new(action: &str) -> Self {
        let protected = matches!(action, "pull" | "pullRef" | "checkout" | "checkoutRemote" | "merge" | "rebase" | "rebaseInteractive" | "cherryPick" | "revert" | "squash" | "gitFlowFinish");
        let mut stages = vec!["preparing"];
        if protected { stages.push("saved"); }
        if matches!(action, "fetch" | "pull" | "pullRef") { stages.push("fetch"); }
        if action != "fetch" { stages.push(if protected { "integration" } else { "operation" }); }
        if protected { stages.push("restore"); }
        stages.push("finished");
        Self { done: vec![false; stages.len()], stages, active: 0, phase: "preparing", percent: 0 }
    }

    pub fn prepared(&mut self) {
        self.done[0] = true;
        let phase = if self.stages.contains(&"fetch") { "fetch" } else if self.stages.contains(&"integration") { "integration" } else { "operation" };
        self.enter(phase);
        self.calculate(0);
    }

    pub fn observe(&mut self, event: &Event) {
        // Commands used to restore a stash belong to that restoration, even if
        // their verbs also occur in the operation itself (checkout/reset/apply).
        if self.phase == "restore" && event.kind != "local-restore" { return; }
        let phase = match event.kind.as_str() {
            "local-restore" => "restore",
            "local-saved" | "stash-push" if self.stages.contains(&"saved") => "saved",
            "fetch" if self.stages.contains(&"fetch") => "fetch",
            "pull" if event.state == "running" => "fetch",
            "pull" | "integration" | "merge" | "rebase" | "replay" | "cherry-pick" | "revert" | "checkout" | "reset" if self.stages.contains(&"integration") => "integration",
            _ if self.stages.contains(&"operation") && !event.command.is_empty() => "operation",
            _ => return,
        };
        if !matches!(event.state.as_str(), "running" | "completed" | "processed" | "skipped") { return; }
        if !self.enter(phase) { return; }
        if event.state != "running" {
            self.done[self.active] = true;
            if event.kind == "pull" { self.complete("fetch"); }
        }
        self.calculate(0);
    }

    pub fn transfer(&mut self, percent: u8) { self.calculate(percent.min(100)); }

    pub fn verifying(&mut self) {
        self.done.fill(true);
        self.enter("finished");
        self.done[self.active] = false;
        self.calculate(0);
    }

    pub fn finish(&mut self, succeeded: bool) {
        if succeeded { self.percent = 100; self.phase = "finished"; }
    }

    fn enter(&mut self, phase: &'static str) -> bool {
        let Some(index) = self.stages.iter().position(|stage| *stage == phase) else { return false; };
        self.active = index;
        self.phase = phase;
        true
    }

    fn complete(&mut self, phase: &str) {
        if let Some(index) = self.stages.iter().position(|stage| *stage == phase) { self.done[index] = true; }
    }

    fn calculate(&mut self, partial: u8) {
        let completed = self.done.iter().filter(|done| **done).count() * 100;
        let partial = if self.done[self.active] { 0 } else { partial as usize };
        self.percent = self.percent.max(((completed + partial) / self.stages.len()).min(99) as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(kind: &str, state: &str) -> Event {
        Event { kind: kind.into(), state: state.into(), command: format!("git {kind}"), ..Default::default() }
    }

    #[test]
    fn commands_without_transfer_percent_advance_only_when_phases_finish() {
        for action in ["fetch", "push", "pushRef", "clone", "submoduleUpdate", "stash"] {
            let mut progress = Progress::new(action);
            assert_eq!(progress.percent, 0);
            progress.prepared();
            assert_eq!(progress.percent, 33, "{action}");
            progress.observe(&command(action, "running"));
            assert_eq!(progress.percent, 33);
            progress.observe(&command(action, "completed"));
            assert_eq!(progress.percent, 66);
            progress.verifying();
            assert_eq!(progress.percent, 66);
            progress.finish(true);
            assert_eq!(progress.percent, 100);
        }
    }

    #[test]
    fn fetch_first_pull_counts_save_integrate_restore_and_verification() {
        let mut progress = Progress::new("pull");
        progress.prepared();
        assert_eq!(progress.percent, 16);
        for (kind, expected) in [("fetch", 33), ("stash-push", 50), ("merge", 66), ("local-restore", 83)] {
            progress.observe(&command(kind, "running"));
            progress.observe(&command(kind, "completed"));
            assert_eq!(progress.percent, expected, "{kind}");
        }
        progress.verifying();
        assert_eq!(progress.percent, 83);
        progress.finish(true);
        assert_eq!(progress.percent, 100);
    }

    #[test]
    fn transfers_do_not_regress_on_git_phase_resets_or_finish_early() {
        let mut progress = Progress::new("fetch");
        progress.prepared();
        progress.transfer(75);
        assert_eq!(progress.percent, 58);
        progress.transfer(0);
        assert_eq!(progress.percent, 58);
        progress.transfer(100);
        assert_eq!(progress.percent, 66);
        progress.finish(false);
        assert_eq!(progress.percent, 66);
    }

    #[test]
    fn skipped_phases_and_restore_commands_keep_progress_in_the_correct_phase() {
        let mut progress = Progress::new("checkout");
        progress.prepared();
        progress.observe(&command("checkout", "completed"));
        assert_eq!(progress.percent, 40);
        progress.observe(&command("local-restore", "running"));
        progress.observe(&command("checkout", "completed"));
        assert_eq!(progress.phase, "restore");
        progress.verifying();
        assert_eq!(progress.percent, 80);
        progress.finish(false);
        assert_eq!(progress.percent, 80);
    }
}
