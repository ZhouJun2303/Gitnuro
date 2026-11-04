<script lang="ts">
  import { scenes, type LabScene } from "./scenes";

  const groups = [...new Set(scenes.map((scene) => scene.group))];

  function inGroup(group: string): LabScene[] {
    return scenes.filter((scene) => scene.group === group);
  }
</script>

<main class="lab-home">
  <header>
    <h1>AweGit 试验台</h1>
    <p>每个场景打开本地 <code>.lab</code> 仓库里的正式界面。桌面应用仍用 <code>pnpm start</code>。</p>
  </header>
  {#each groups as group (group)}
    <section>
      <h2>{group}</h2>
      <div class="lab-grid">
        {#each inGroup(group) as scene (scene.id)}
          <a href={scene.href} data-sveltekit-reload>{scene.title}</a>
        {/each}
      </div>
    </section>
  {/each}
</main>

<style>
  .lab-home {
    height: 100%;
    overflow: auto;
    padding: 32px 40px 64px;
    background: var(--canvas);
    color: var(--text);
  }

  header {
    max-width: 720px;
    margin-bottom: 28px;
  }

  h1 {
    margin: 0 0 8px;
    font-size: 28px;
  }

  p, h2 {
    color: var(--text-secondary);
  }

  h2 {
    margin: 24px 0 10px;
    font-size: 13px;
    letter-spacing: 0.04em;
  }

  .lab-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  a {
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--elevated);
    color: var(--text);
    text-decoration: none;
  }

  a:hover {
    border-color: var(--accent);
  }

  code {
    font-family: var(--mono);
  }
</style>
