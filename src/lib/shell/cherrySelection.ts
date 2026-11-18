import type { ShellState } from "./state.svelte";

export function previewCherrySelection(shell: ShellState, commits: readonly { id: string; summary: string }[], record: boolean) {
  const revs = commits.map((commit) => commit.id);
  if (revs.length === 0) return;
  const count = String(revs.length);
  const path = shell.repoPath(), openGen = shell.openGen;
  const note = record ? ` ${shell.tr("dialog.cherryPickRecordNote")}` : "";
  shell.ask(
    shell.tr(record ? "menu.cherryPickSelectedRecord" : "menu.cherryPickSelected", { count }),
    `${shell.tr("dialog.cherryPickSelectedBody", { count })}${note}`,
    () => {
      if (shell.openGen === openGen && shell.viewing(path)) void shell.mutate({ action: "cherryPick", revs, record });
    },
    [],
    {
      subject: shell.tr("menu.selectedCommits", { count }),
      detail: commits.map((commit) => `${commit.id.slice(0, 7)} ${commit.summary}`).join("\n"),
    },
  );
}
