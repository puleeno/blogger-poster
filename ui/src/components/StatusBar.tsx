import type { OAuthStatus, TabInfo } from "../api";

export function StatusBar({
  chromeConnected,
  port,
  oauth,
  scenariosCount,
  onOpenSettings,
}: {
  chromeConnected: boolean;
  port: number;
  oauth?: OAuthStatus;
  scenariosCount: number;
  onOpenSettings: () => void;
}) {
  return (
    <footer className="bp-statusbar">
      <span className={`bp-dot ${chromeConnected ? "ok" : "off"}`} />
      <span>
        Chrome {chromeConnected ? `OK (port ${port})` : "chưa kết nối"}
      </span>
      <span className="bp-sb-sep">•</span>
      <span className={`bp-dot ${oauth?.has_tokens ? "ok" : oauth?.configured ? "warn" : "off"}`} />
      <span>
        Blogger {oauth?.has_tokens ? "đã đăng nhập" : oauth?.configured ? "chưa đăng nhập" : "chưa cấu hình"}
      </span>
      <span className="bp-sb-sep">•</span>
      <span>{scenariosCount} kịch bản</span>
      <span className="bp-tb-spacer" />
      <button className="bp-link-btn" onClick={onOpenSettings}>
        Cài đặt ⚙
      </button>
    </footer>
  );
}