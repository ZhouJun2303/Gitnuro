import { expect, test } from "@playwright/test";
import type { Account, Settings } from "../src/lib/status";
import { scenes } from "../src/lib/lab/scenes";
import { openScene, pinLabSettings } from "./helpers";

const account = (id: string, name: string): Account => ({ id, name, forge: "github", label: name, token: "", host: "github.com", clientId: "", login: id, email: "", avatarUrl: "", defaultForHost: false });

for (const theme of ["apple", "light", "dark"] as const) {
  for (const signOut of [false, true]) {
    test(`account sync ${signOut ? "preserves a sign-out requested during discovery" : "adopts saved removals without another save"} (${theme})`, async ({ page }) => {
      await pinLabSettings(page, { locale: "en", theme });
      let started = false, release!: () => void;
      const gate = new Promise<void>((done) => { release = done; });
      const saves: Settings[] = [];
      await page.route("**/__lab/invoke", async (route) => {
        const { cmd, args } = route.request().postDataJSON();
        if (cmd === "save_settings" && started) {
          saves.push(args.values);
          return route.fulfill({ json: { ok: true, value: args.values } });
        }
        if (cmd !== "sync_accounts") return route.fallback();
        const payload = await (await route.fetch()).json();
        payload.value = {
          ...payload.value,
          githubToken: "", gitlabToken: "", bitbucketToken: "", azureToken: "",
          accounts: [
            ...(signOut ? [account("lab-github", "Refreshed profile"), account("lab-gitlab", "Account signed out locally")] : []),
            account("audit-discovered", "Discovered account"),
          ],
        };
        started = true;
        if (signOut) await gate;
        return route.fulfill({ json: payload });
      });
      await openScene(page, scenes.find((scene) => scene.id === "prefs-accounts")!, `&theme=${theme}&locale=en`);
      const dialog = page.locator('[data-dialog="prefs"]');
      const sync = dialog.getByRole("button", { name: "Sync accounts already signed in on this machine", exact: true });
      await expect(dialog.locator("[data-account-card]")).toHaveCount(2);
      await sync.click();
      try {
        await expect.poll(() => started).toBe(true);
        if (signOut) {
          await dialog.locator('[data-account-card="lab-gitlab"]').getByRole("button", { name: "Sign out", exact: true }).click();
          await page.getByRole("alertdialog").getByRole("button", { name: "Confirm", exact: true }).click();
          await expect(dialog.locator('[data-account-card="lab-gitlab"]')).toHaveCount(0);
          await expect.poll(() => saves.length).toBe(1);
        }
        release();
        await expect(dialog.locator('[data-account-card="audit-discovered"]')).toBeVisible();
        await expect(sync).toBeEnabled();
        await expect(dialog.locator('[data-account-card="lab-gitlab"]')).toHaveCount(0);
        await expect(dialog.locator("[data-account-card]")).toHaveCount(signOut ? 2 : 1);
        if (signOut) {
          expect(saves).toHaveLength(2);
          expect(saves.at(-1)!.accounts.map((row) => row.id)).toEqual(["lab-github", "audit-discovered"]);
          await expect(dialog.locator('[data-account-card="lab-github"]')).toContainText("Refreshed profile");
        } else expect(saves).toHaveLength(0);
      } finally { release(); }
    });
  }
}
