/** Merge a response against its captured input. Edits made during the request win. */
export function mergeFields<T extends object>(current: T, before: T, incoming: T, same: (a: unknown, b: unknown) => boolean): T {
  const result = { ...incoming };
  for (const key of Object.keys(current) as Array<keyof T>) {
    if (!same(current[key], before[key])) result[key] = current[key];
  }
  return result;
}

export function mergeRecords<T extends { id: string }>(current: readonly T[], before: readonly T[], incoming: readonly T[], same: (a: unknown, b: unknown) => boolean): T[] {
  const previous = new Map(before.map((row) => [row.id, row]));
  const received = new Map(incoming.map((row) => [row.id, row]));
  const result = current.map((row) => {
    const base = previous.get(row.id), updated = received.get(row.id);
    return base && updated ? mergeFields(row, base, updated, same) : row;
  });
  const present = new Set(current.map((row) => row.id));
  for (const row of incoming) if (!present.has(row.id) && !previous.has(row.id)) result.push(row);
  return result;
}

/** Three-way settings responses also preserve edits inside record collections. */
export function mergeResponse<T>(current: T, before: T, incoming: T, same: (a: unknown, b: unknown) => boolean): T {
  if (same(current, before)) return incoming;
  const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object" && !Array.isArray(value);
  if (Array.isArray(current) && Array.isArray(before) && Array.isArray(incoming)) {
    if (![...current, ...before, ...incoming].every((row) => record(row) && typeof row.id === "string")) return current;
    const base = new Map(before.map((row) => [row.id, row]));
    const received = new Map(incoming.map((row) => [row.id, row]));
    const rows = current.flatMap((row) => {
      if (!base.has(row.id)) return [row];
      if (!received.has(row.id)) return same(row, base.get(row.id)) ? [] : [row];
      return [mergeResponse(row, base.get(row.id), received.get(row.id), same)];
    });
    const present = new Set(current.map((row) => row.id));
    for (const row of incoming) if (!present.has(row.id) && !base.has(row.id)) rows.push(row);
    return rows as T;
  }
  if (record(current) && record(before) && record(incoming)) {
    const result: Record<string, unknown> = { ...incoming };
    for (const key of Object.keys(current)) result[key] = mergeResponse(current[key], before[key], incoming[key], same);
    return result as T;
  }
  return current;
}
