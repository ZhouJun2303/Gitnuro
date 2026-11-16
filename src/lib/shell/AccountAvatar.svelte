<script lang="ts">
  import { untrack } from "svelte";
  import { cachedAvatar, loadAvatar } from "./avatars";

  let { url = "", initial = "?" }: { url?: string; initial?: string } = $props();
  let src = $state<string | null>(untrack(() => (url ? cachedAvatar(url) : null)));

  $effect(() => {
    const target = url;
    const hit = target ? cachedAvatar(target) : null;
    src = hit;
    if (!target || hit) return;
    let alive = true;
    void loadAvatar(target).then((value) => {
      if (alive) src = value;
    });
    return () => {
      alive = false;
    };
  });
</script>

{#if src}
  <img class="account-avatar" alt="" src={src} />
{:else}
  <span class="account-avatar" aria-hidden="true">{initial}</span>
{/if}
