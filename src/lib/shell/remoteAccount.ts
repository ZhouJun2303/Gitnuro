import type { Account } from "../status";

export type RemoteAccountPreview = {
  protocol: string;
  host: string;
  path: string;
  username: string;
  ssh: boolean;
  account: Account | null;
  reason: "override" | "login" | "default" | "only" | "none";
  choices: Account[];
};

/** The account a remote URL will use. Same order as the git credential helper. */
export function resolveRemoteAccount(accounts: Account[], url: string, pick = ""): RemoteAccountPreview {
  const parts = parseRemote(url);
  const host = hostKey(parts.host);
  const choices = hostSafe(host) ? accounts.filter((item) => hostsMatch(item, host)) : [];
  const chosen = choices.find((item) => item.id === pick);
  const byLogin = parts.username
    ? choices.find((item) => item.login.trim().toLowerCase() === parts.username.toLowerCase())
    : undefined;
  const fallback = choices.find((item) => item.defaultForHost) ?? (choices.length === 1 ? choices[0] : undefined);
  const account = chosen ?? byLogin ?? fallback ?? null;
  const reason = chosen ? "override" : byLogin ? "login" : account && account.defaultForHost ? "default" : account ? "only" : "none";
  return {
    protocol: protocolLabel(parts.protocol),
    host,
    path: parts.path,
    username: parts.username,
    ssh: parts.protocol === "ssh",
    account,
    reason,
    choices,
  };
}

function protocolLabel(protocol: string) {
  if (protocol === "https") return "HTTPS";
  if (protocol === "http") return "HTTP";
  if (protocol === "ssh") return "SSH";
  return "";
}

function parseRemote(url: string) {
  const text = url.trim();
  if (text.toLowerCase().startsWith("ssh://")) return parseAuthority("ssh", text.slice(6));
  const schemeAt = text.indexOf("://");
  if (schemeAt > 0) {
    const scheme = text.slice(0, schemeAt).toLowerCase();
    const protocol = scheme === "https" || scheme === "http" || scheme === "ssh" ? scheme : "other";
    return parseAuthority(protocol, text.slice(schemeAt + 3));
  }
  const at = text.indexOf("@");
  const colon = text.indexOf(":", at + 1);
  if (at > 0 && colon > at && !text.slice(at + 1, colon).includes("/")) {
    return { protocol: "ssh", host: hostOnly(text.slice(at + 1, colon)), username: ignoreGit(text.slice(0, at)), path: cleanPath(text.slice(colon + 1)) };
  }
  return { protocol: "other", host: "", username: "", path: "" };
}

function parseAuthority(protocol: string, rest: string) {
  const at = rest.lastIndexOf("@");
  const user = at >= 0 ? ignoreGit(rest.slice(0, at)) : "";
  const hostpart = at >= 0 ? rest.slice(at + 1) : rest;
  const slash = hostpart.search(/[/?]/);
  const host = slash >= 0 ? hostpart.slice(0, slash) : hostpart;
  const path = slash >= 0 ? cleanPath(hostpart.slice(slash + 1)) : "";
  return { protocol, host: hostOnly(host), username: user, path };
}

function cleanPath(path: string) {
  return path.replace(/[?#].*$/, "").replace(/\.git$/i, "").replace(/^\/+|\/+$/g, "");
}

function ignoreGit(user: string) {
  const name = user.split(":")[0]?.trim() ?? "";
  if (!name || name.toLowerCase() === "git") return "";
  return name;
}

function hostOnly(host: string) {
  let value = host.trim().replace(/^\[|\]$/g, "");
  const colon = value.lastIndexOf(":");
  if (colon > 0 && /^\d+$/.test(value.slice(colon + 1))) value = value.slice(0, colon);
  return value.replace(/\.+$/g, "");
}

function hostKey(host: string) {
  const value = hostOnly(host).toLowerCase();
  if (value === "www.github.com" || value === "ssh.github.com" || value === "gist.github.com") return "github.com";
  if (value === "www.gitlab.com") return "gitlab.com";
  if (value === "www.bitbucket.org") return "bitbucket.org";
  return value;
}

function defaultHost(forge: string) {
  switch (forge) {
    case "gitlab": return "gitlab.com";
    case "bitbucket": return "bitbucket.org";
    case "azure": return "dev.azure.com";
    case "gitee": return "gitee.com";
    case "atomgit": return "atomgit.com";
    case "gitcode": return "gitcode.com";
    case "codeup": return "codeup.aliyun.com";
    case "cnb": return "cnb.cool";
    case "gitea": return "codeberg.org";
    case "sourcehut": return "git.sr.ht";
    case "github": return "github.com";
    default: return "";
  }
}

function hostSafe(host: string) {
  return host.length > 0
    && host.length < 200
    && !host.startsWith("-")
    && !host.startsWith(".")
    && !host.endsWith(".")
    && !host.includes("..")
    && /^[a-z0-9.-]+$/.test(host);
}

function hostsMatch(account: Account, remoteHost: string) {
  const remote = hostKey(remoteHost);
  if (!hostSafe(remote)) return false;
  const own = account.host.trim() ? hostKey(account.host) : defaultHost(account.forge);
  if (own && own === remote) return true;
  if (account.forge === "azure") {
    const pub = !account.host.trim() || hostKey(account.host) === "dev.azure.com";
    if (pub && (remote === "dev.azure.com" || remote.endsWith(".visualstudio.com"))) return true;
  }
  return false;
}
