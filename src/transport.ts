import { invoke as nativeInvoke } from "@tauri-apps/api/core";
export const isWeb = import.meta.env.VITE_WEB === "1";
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isWeb) return nativeInvoke<T>(cmd, args);
  if (cmd === "set_orientation" || cmd === "plugin:app|exit") return undefined as T;
  if (cmd === "open_url") {
    const url = new URL(String(args?.url));
    if (!["http:", "https:"].includes(url.protocol)) throw "不支持的链接";
    window.open(url.href, "_blank", "noopener,noreferrer");
    return undefined as T;
  }
  const res = await fetch(`/api/invoke/${encodeURIComponent(cmd)}`, {
    method: "POST", credentials: "same-origin",
    headers: { "Content-Type": "application/json", "X-HG-Request": "1" },
    body: JSON.stringify(args ?? {}),
  });
  if (res.status === 401) { location.assign("/login"); throw "请先登录"; }
  const body = await res.json();
  if (!res.ok) throw body.error ?? "请求失败";
  return body as T;
}
