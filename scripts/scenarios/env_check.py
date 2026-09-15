"""In trạng thái OAuth / thư mục dữ liệu (kiểm tra cấu hình)."""
META = {
    "name": "env_check",
    "description": "In thông tin môi trường: appdata dir, scenarios dir, trạng thái OAuth, trạng thái Chrome.",
    "params": [],
}

def scenario(params):
    import blogger_automation as b
    out = {
        "app_data_dir": b.app_data_dir(),
        "scenario_dir": b.scenario_dir(),
        "oauth": b.oauth_status(),
    }
    try:
        out["chrome_connected"] = b.chrome_connected()
        out["tabs"] = [{"id": t.id, "title": t.title} for t in b.chrome_list_tabs()]
    except Exception as exc:
        out["chrome_connected"] = False
        out["tabs"] = []
        out["chrome_error"] = str(exc)
    return out