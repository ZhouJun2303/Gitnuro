export type CommitSelectionEvent = Pick<MouseEvent, "ctrlKey" | "metaKey" | "shiftKey">;

export class CommitSelection {
  ids = $state.raw<ReadonlySet<string> | null>(null);
  anchor: string | null = null;
  private rows: readonly { id: string }[] | null = null;
  private visible = new Set<string>();

  has(id: string, active: string | null) {
    return this.ids?.has(id) ?? active === id;
  }

  clear() {
    this.ids = null;
    this.anchor = null;
  }

  retain(rows: readonly { id: string }[], active: string | null) {
    if (this.rows !== rows) {
      this.rows = rows;
      this.visible = new Set(rows.map((row) => row.id));
    }
    const selected = this.ids ?? new Set(active ? [active] : []);
    if (selected.size === 0) return;
    const kept = new Set([...selected].filter((id) => this.visible.has(id)));
    if (kept.size !== selected.size) this.ids = kept;
    if (this.anchor && !this.visible.has(this.anchor)) this.anchor = null;
  }

  select(id: string, active: string | null, rows: readonly { id: string }[], event?: CommitSelectionEvent): string | null {
    this.retain(rows, active);
    const toggle = event?.ctrlKey || event?.metaKey;
    const selected = new Set(this.ids ?? (active ? [active] : []));
    if (event?.shiftKey) {
      const anchor = this.anchor ?? active ?? id;
      const from = rows.findIndex((row) => row.id === anchor);
      const to = rows.findIndex((row) => row.id === id);
      if (from >= 0 && to >= 0) {
        const range = rows.slice(Math.min(from, to), Math.max(from, to) + 1).map((row) => row.id);
        this.ids = new Set(toggle ? [...selected, ...range] : range);
        this.anchor = anchor;
        return id;
      }
    }
    this.anchor = id;
    if (!toggle) {
      this.ids = new Set([id]);
      return id;
    }
    if (selected.has(id)) selected.delete(id);
    else selected.add(id);
    this.ids = selected;
    if (selected.has(id)) return id;
    if (active && selected.has(active)) return active;
    return [...selected].at(-1) ?? null;
  }
}
