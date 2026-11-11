<script lang="ts">
  import { getShell } from "./context";
  import PreferenceControls from "../PreferenceControls.svelte";
  import type { PreferenceKey } from "../preferences";
  let { keys, label = "", children }: { keys: PreferenceKey[]; label?: string; children?: import("svelte").Snippet } = $props();
  const shell = getShell();
</script>

<PreferenceControls values={shell.settings} {keys} tr={(key) => shell.tr(key)} {label} onchange={(patch) => void shell.updatePreferences(patch)} {children} />
