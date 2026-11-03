import { getContext, setContext } from "svelte";
import type { ShellState } from "./state.svelte";

const KEY = Symbol("awegit-shell");

export function setShell(shell: ShellState) {
  setContext(KEY, shell);
}

export function getShell(): ShellState {
  const shell = getContext<ShellState>(KEY);
  if (!shell) throw new Error("Shell state is only available inside the shell");
  return shell;
}
