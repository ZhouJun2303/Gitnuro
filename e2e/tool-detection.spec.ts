import { expect, test, type Page } from "@playwright/test";
import { pinLabSettings } from "./helpers";

const commonTools = [
  { name: "Beyond Compare", path: "c:/program files/beyond compare 4/BCompare.exe", diffArgs: '"{left}" "{right}"', mergeArgs: '"{ours}" "{theirs}" "{base}" "{working}"' },
  { name: "WinMerge", path: "D:\\Portable Tools\\WinMergeU.exe", diffArgs: '"{left}" "{right}"', mergeArgs: '-e -u -o "{working}" "{ours}" "{base}" "{theirs}"' },
  { name: "Code Compare", path: "D:\\Tools\\CodeCompare.exe", diffArgs: '"{left}" "{right}"', mergeArgs: "" },
  { name: "Code Merge", path: "D:\\Tools\\CodeMerge.exe", diffArgs: "", mergeArgs: '-MF="{ours}" -BF="{base}" -TF="{theirs}" -RF="{working}"' },
];
const editors = [
  { name: "Visual Studio Code", path: "c:/program files/microsoft vs code/CODE.exe", args: '-g "{file}:{line}"' },
  { name: "Notepad++", path: "D:\\Portable Tools\\notepad++.exe", args: '-n{line} "{file}"' },
  { name: "Zed", path: "D:\\Tools\\Zed.exe", args: '"{file}:{line}"' },
];
const themes = [
  { name: "apple", locale: "zh" },
  { name: "light", locale: "zh" },
  { name: "dark", locale: "en" },
] as const;
type Kind = "diff" | "merge" | "editor";
type SavedTool = { id: string; name: string; path: string; args: string; extensions: string[]; fallback: boolean };

async function openTools(page: Page, kind: Kind, theme: typeof themes[number]) {
  await pinLabSettings(page, {});
  const scene = kind === "diff" ? "prefs-integration" : `prefs-integration-${kind}`;
  await page.goto(`/?scene=${scene}&theme=${theme.name}&locale=${theme.locale}`);
  await expect(page.locator(`[data-lab-scene="${scene}"]`)).toBeVisible();
  await expect(page.locator(".tool-tabs")).toHaveAttribute("data-tool-kind", kind);
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme.name);
}

for (const theme of themes) {
  test.describe(theme.name, () => {
    for (const kind of ["diff", "merge", "editor"] as const) {
      test(`tool detection imports only selected ${kind} tools and preserves existing configuration`, async ({ page }) => {
        let saved: Record<string, any> | undefined;
        let scans = 0;
        await pinLabSettings(page, {});
        await page.route("**/__lab/invoke", async (route) => {
          const body = route.request().postDataJSON();
          if (body.cmd === "save_settings") saved = body.args.values;
          if (body.cmd === "common_tools" || body.cmd === "editor_tools") {
            scans += 1;
            return route.fulfill({ json: { ok: true, value: body.cmd === "editor_tools" ? editors : commonTools } });
          }
          return route.fallback();
        });
        await openTools(page, kind, theme);
        expect(scans).toBe(0);
        const existingCount = await page.locator(".tool-list > button").count();
        await page.locator(".tool-detect").click();
        const picker = page.locator('[data-dialog="editorImport"]');
        const expectedTitle = kind === "editor" ? /识别文本编辑工具|Detect Text Editors/ : kind === "diff" ? /识别外部差异工具|Detect External Diff Tools/ : /识别外部合并工具|Detect External Merge Tools/;
        await expect(picker.locator("h2")).toHaveText(expectedTitle);
        await expect(picker.locator(".editor-candidate")).toHaveCount(3);
        const existing = picker.locator(".editor-candidate").filter({ hasText: kind === "editor" ? "Visual Studio Code" : "Beyond Compare" }).locator("input");
        await expect(existing).toBeDisabled();
        await expect(existing).toBeChecked();
        const selectedName = kind === "editor" ? "Notepad++" : "WinMerge";
        const selected = picker.locator(".editor-candidate").filter({ hasText: selectedName }).locator("input");
        const submit = picker.locator('button[type="submit"]');
        await expect(submit).toBeDisabled();
        await selected.check();
        await picker.getByRole("button", { name: /重新识别|Detect again/ }).click();
        await expect.poll(() => scans).toBe(2);
        await expect(selected).toBeChecked();
        await expect(submit).toBeEnabled();
        await submit.click();
        await expect(page.locator(".tool-tabs")).toHaveAttribute("data-tool-kind", kind);
        await expect(page.locator(".tool-list > button")).toHaveCount(existingCount + 1);
        await expect(page.locator(".tool-list > button.on")).toContainText(selectedName);
        const key = `${kind}Tools`;
        await expect.poll(() => saved?.[key]?.length).toBe(existingCount + 1);
        const imported = (saved![key] as SavedTool[]).find((tool) => tool.name === selectedName)!;
        expect(imported.path).toBe(kind === "editor" ? editors[1].path : commonTools[1].path);
        expect(imported.args).toBe(kind === "editor" ? editors[1].args : kind === "diff" ? commonTools[1].diffArgs : commonTools[1].mergeArgs);
        expect(imported.extensions).toEqual([]);
        expect(imported.fallback).toBe(false);
        for (const other of ["diff", "merge", "editor"] as const) {
          const list = saved![`${other}Tools`] as SavedTool[];
          expect(list.filter((tool) => tool.fallback)).toHaveLength(1);
          if (other !== "editor") expect(list.find((tool) => tool.id === `lab-${other}-xlsx`)?.extensions).toEqual(["xlsx", "xls"]);
          if (other !== kind) expect(list.some((tool) => tool.path === imported.path)).toBe(false);
        }
        if (kind !== "editor") {
          expect(saved![`${kind}ToolPath`]).toBe("C:\\Program Files\\Beyond Compare 4\\BCompare.exe");
          expect(saved![`${kind}Tool`]).toContain("BCompare.exe");
        }
        await page.locator(".tool-detect").click();
        const added = picker.locator(".editor-candidate").filter({ hasText: selectedName }).locator("input");
        await expect(added).toBeChecked();
        await expect(added).toBeDisabled();
        await expect(picker.locator('button[type="submit"]')).toBeDisabled();
        await page.keyboard.press("Escape");
        await expect(page.locator(".tool-tabs")).toHaveAttribute("data-tool-kind", kind);
        await expect(page.locator(".tool-list > button")).toHaveCount(existingCount + 1);
        if (kind !== "editor") {
          await page.locator(".tool-import").click();
          await expect(page.locator('[data-layer="menu"]')).toContainText(/Fork/);
        }
      });
    }

    test("tool detection can retry failures and empty results without saving", async ({ page }) => {
      let scans = 0;
      let saves = 0;
      await pinLabSettings(page, {});
      await page.route("**/__lab/invoke", async (route) => {
        const body = route.request().postDataJSON();
        if (body.cmd === "save_settings") saves += 1;
        if (body.cmd === "common_tools") {
          scans += 1;
          return route.fulfill({ json: scans === 1 ? { ok: false, error: "Detection unavailable" } : { ok: true, value: [] } });
        }
        return route.fallback();
      });
      await openTools(page, "merge", theme);
      const initialSaves = saves;
      await page.locator(".tool-detect").click();
      const picker = page.locator('[data-dialog="editorImport"]');
      await expect(picker.getByRole("alert")).toHaveText("Detection unavailable");
      await picker.getByRole("button", { name: /重新识别|Detect again/ }).click();
      await expect(picker).toContainText(/未找到已安装的工具|No installed tools found/);
      await expect(picker.getByRole("alert")).toHaveCount(0);
      await expect(picker.locator('button[type="submit"]')).toBeDisabled();
      await picker.locator(".dialog-close").click();
      await expect(page.locator(".tool-tabs")).toHaveAttribute("data-tool-kind", "merge");
      expect(scans).toBe(2);
      expect(saves).toBe(initialSaves);
    });

    test("tool detection ignores a late result after closing and switching kinds", async ({ page }) => {
      let finishOld!: () => void;
      const held = new Promise<void>((resolve) => { finishOld = resolve; });
      let oldFinished!: () => void;
      const finished = new Promise<void>((resolve) => { oldFinished = resolve; });
      let scans = 0;
      await pinLabSettings(page, {});
      await page.route("**/__lab/invoke", async (route) => {
        if (route.request().postDataJSON().cmd !== "common_tools") return route.fallback();
        scans += 1;
        if (scans === 1) {
          await held;
          await route.fulfill({ json: { ok: true, value: [{ ...commonTools[1], name: "Old diff tool" }] } });
          oldFinished();
          return;
        }
        return route.fulfill({ json: { ok: true, value: [{ ...commonTools[1], name: "New merge tool" }] } });
      });
      try {
        await openTools(page, "diff", theme);
        await page.locator(".tool-detect").click();
        const picker = page.locator('[data-dialog="editorImport"]');
        await expect.poll(() => scans).toBe(1);
        await expect(picker).toContainText(/正在识别|Detecting/);
        await picker.getByRole("button", { name: /取消|Cancel/, exact: true }).click();
        await page.locator(".tool-tabs button").nth(1).click();
        await page.locator(".tool-detect").click();
        await expect(picker.locator(".editor-candidate-name")).toHaveText("New merge tool");
        finishOld();
        await finished;
        await expect(picker.locator(".editor-candidate-name")).toHaveText("New merge tool");
        await expect(picker).not.toContainText("Old diff tool");
        await picker.locator(".dialog-close").click();
        await expect(page.locator(".tool-tabs")).toHaveAttribute("data-tool-kind", "merge");
      } finally { finishOld(); }
    });
  });
}
