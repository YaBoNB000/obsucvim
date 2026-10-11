/** 会话令牌（服务端签名发放，浏览器仅存储） */
const KEY = "obsucvim.token";

export function getToken(): string | null {
  try {
    return localStorage.getItem(KEY);
  } catch {
    return null;
  }
}

export function setToken(token: string) {
  try {
    localStorage.setItem(KEY, token);
  } catch {
    /* ignore */
  }
}

export function clearToken() {
  try {
    localStorage.removeItem(KEY);
    // 清理旧演示键
    localStorage.removeItem("obsucvim.user");
    localStorage.removeItem("obsucvim.users");
    localStorage.removeItem("obsucvim.authlock");
    localStorage.removeItem("obsucvim.quota");
  } catch {
    /* ignore */
  }
}

export function authHeaders(): Record<string, string> {
  const t = getToken();
  return t ? { authorization: `Bearer ${t}` } : {};
}
