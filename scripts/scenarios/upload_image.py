"""Upload 1 ảnh qua Chrome (form file input) và trả về URL Blogger."""
META = {
    "name": "upload_image",
    "description": "Mở editor blogger (nếu cần), upload file ảnh bằng CDP, trả về URL CDN.",
    "params": [{"name": "path", "type": "string"}, {"name": "timeout_ms", "type": "number", "default": 120000}],
}

def scenario(params):
    import blogger_automation as b
    path = params["path"]
    blog_id = params.get("blog_id")
    if blog_id:
        target = b.chrome_ensure_editor(blog_id, params.get("editor_mode") or "ide")
        b.chrome_wait_ready(target)
        target_id = target
    else:
        target_id = None
    url = b.chrome_upload_image(target_id, path, params.get("timeout_ms") or 120000)
    return {"url": url, "ok": url.startswith("http")}