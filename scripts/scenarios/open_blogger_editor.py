"""Mở (reuse) tab editor Blogger trong Chrome qua CDP."""
META = {
    "name": "open_blogger_editor",
    "description": "Mở trang soạn bài của Blogger trong Chrome và chờ sẵn sàng.",
    "params": [],
}

def scenario(params):
    import blogger_automation as b
    blog_id = params.get("blog_id")
    if not blog_id:
        raise ValueError("cần tham số blog_id")
    target = b.chrome_ensure_editor(blog_id, params.get("editor_mode") or "ide")
    b.chrome_wait_ready(target)
    return {"target_id": target, "ok": True}