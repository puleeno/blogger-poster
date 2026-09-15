import { useEffect, useState } from "react";
import { api, type OAuthStatus, type Settings } from "../api";

export function SettingsModal({
  open,
  settings,
  onClose,
  onSettings,
  refreshAll,
}: {
  open: boolean;
  settings: Settings | null;
  onClose: () => void;
  onSettings: (patch: Partial<Settings>) => void;
  refreshAll: () => Promise<void>;
}) {
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [oauth, setOauth] = useState<OAuthStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<"oauth" | "chrome" | "general">("oauth");

  useEffect(() => {
    if (!open) return;
    setTab("oauth");
    api.oauthGetConfig().then((cfg) => {
      setClientId(cfg.client_id || "");
      setClientSecret(cfg.client_secret || "");
    });
    api.oauthStatus().then(setOauth).catch(() => {});
  }, [open]);

  if (!open) return null;

  const saveOauth = async () => {
    setBusy(true);
    try {
      await api.oauthSetConfig(clientId, clientSecret);
      const st = await api.oauthStatus();
      setOauth(st);
      alert("Lưu cấu hình OAuth thành công.");
    } catch (e) {
      alert(String(e));
    } finally {
      setBusy(false);
    }
  };

  const startLogin = async () => {
    setBusy(true);
    try {
      await api.oauthSetConfig(clientId, clientSecret);
      const res = await api.oauthLoginStart();
      if (res.manual) {
        alert(`Chưa kết nối Chrome. Mở link này bằng tay:\n\n${res.url}`);
      }
      // poll until tokens arrive
      const poll = setInterval(async () => {
        const st = await api.oauthStatus();
        setOauth(st);
        if (st.has_tokens) {
          clearInterval(poll);
          alert("Đã đăng nhập Blogger thành công!");
        }
      }, 2000);
      setTimeout(() => clearInterval(poll), 120000);
    } catch (e) {
      alert(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="bp-modal-mask" onClick={onClose}>
      <div className="bp-modal" onClick={(e) => e.stopPropagation()}>
        <header className="bp-modal-head">
          <h3>Cài đặt</h3>
          <button className="bp-close" onClick={onClose}>
            ✕
          </button>
        </header>

        <nav className="bp-tabs">
          <button className={tab === "oauth" ? "active" : ""} onClick={() => setTab("oauth")}>
            Blogger / OAuth
          </button>
          <button className={tab === "chrome" ? "active" : ""} onClick={() => setTab("chrome")}>
            Chrome
          </button>
          <button className={tab === "general" ? "active" : ""} onClick={() => setTab("general")}>
            Chung
          </button>
        </nav>

        {tab === "oauth" && (
          <div className="bp-modal-body">
            <p className="bp-hint">
              Tạo OAuth client type &quot;Desktop app&quot; tại Google Cloud Console
              (scope: <code>https://www.googleapis.com/auth/blogger</code>) và dán client id/secret.
            </p>
            <label>
              Client ID
              <input value={clientId} onChange={(e) => setClientId(e.target.value)} placeholder="xxx.apps.googleusercontent.com" />
            </label>
            <label>
              Client Secret
              <input value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} placeholder="GOCSPX-..." type="password" />
            </label>

            <div className="bp-row">
              <button className="bp-btn" onClick={saveOauth} disabled={busy}>
                {busy ? "Đang lưu…" : "Lưu cấu hình"}
              </button>
              <button className="bp-btn primary" onClick={startLogin} disabled={busy}>
                {oauth?.has_tokens ? "Đăng nhập lại" : "Đăng nhập Google"}
              </button>
              {oauth?.has_tokens && (
                <button
                  className="bp-btn danger"
                  onClick={async () => {
                    await api.oauthLogout();
                    setOauth(await api.oauthStatus());
                  }}
                >
                  Đăng xuất
                </button>
              )}
            </div>
            <div className="bp-hint">
              Trạng thái:{" "}
              {oauth?.configured ? "đã cấu hình" : "chưa cấu hình"}
              {oauth?.has_tokens ? " • đã có token" : ""}
              {oauth?.expired && " • token hết hạn (tự refresh)"}
            </div>
          </div>
        )}

        {tab === "chrome" && (
          <div className="bp-modal-body">
            <p className="bp-hint">
              App điều khiển Chrome đang chạy qua CDP (Chrome DevTools Protocol). Bạn có thể tự
              mở Chrome với <code>--remote-debugging-port</code>, hoặc dùng nút bên dưới để app tự mở
              profile riêng cho editor.
            </p>
            <label>
              Cổng remote debugging
              <input
                type="number"
                value={settings?.chrome_port ?? 9222}
                onChange={(e) => onSettings({ chrome_port: Number(e.target.value) })}
              />
            </label>
            <label>
              Kiểu editor Blogger
              <select
                value={settings?.editor_mode ?? "ide"}
                onChange={(e) => onSettings({ editor_mode: e.target.value })}
              >
                <option value="ide">IDE (preview)</option>
                <option value="classic">Classic</option>
              </select>
            </label>
            <div className="bp-row">
              <button
                className="bp-btn primary"
                onClick={async () => {
                  await api.chromeLaunch(settings?.chrome_port ?? 9222);
                  alert("Chrome đã khởi động với profile editor. Hãy đăng nhập Blogger trong Chrome này!");
                }}
              >
                Mở Chrome (profile editor)
              </button>
              <button
                className="bp-btn"
                onClick={async () => {
                  await api.chromeConnect(settings?.chrome_port ?? 9222);
                  alert("Đã kết nối Chrome");
                }}
              >
                Kết nối Chrome
              </button>
            </div>
          </div>
        )}

        {tab === "general" && (
          <div className="bp-modal-body">
            <label>
              Thời gian chờ upload ảnh (ms)
              <input
                type="number"
                value={settings?.image_timeout_ms ?? 120000}
                onChange={(e) => onSettings({ image_timeout_ms: Number(e.target.value) })}
              />
            </label>
            <button
              className="bp-btn"
              onClick={async () => {
                await refreshAll();
                alert("Đã refresh cache tags / posts / settings.");
              }}
            >
              Refresh toàn bộ cache
            </button>
          </div>
        )}
      </div>
    </div>
  );
}