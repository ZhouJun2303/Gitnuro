<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getShell } from "./context";

  let { branch = "", message }: { branch?: string; message: string } = $props();
  const shell = getShell();
</script>

{#if branch}
  <span class="name-plate" style:--lane="var(--ref-local)" title={branch}><span>{branch}</span></span>
{/if}
<span class="name-plate grow" style:--lane="var(--ref-stash)" title={message}>
  <span>
    {#each shell.linkParts(message) as part, index (`${index}-${part.text}`)}
      {#if part.href}
        <a class="issue" href={part.href} onclick={(event) => { event.preventDefault(); event.stopPropagation(); void openUrl(part.href); }}>{part.text}</a>
      {:else if shell.settings.highlightIssues && (/^#\d+$/.test(part.text) || part.text.startsWith("http"))}
        <span class="issue">{part.text}</span>
      {:else}
        {part.text}
      {/if}
    {/each}
  </span>
</span>
