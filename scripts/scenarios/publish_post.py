"""Tạo / cập nhật và xuất bản một bài viết lên Blogger."""
META = {
    "name": "publish_post",
    "description": "Tạo mới (hoặc cập nhật) và xuất bản bài viết với title/content/labels.",
    "params": [
        {"name": "blog_id", "type": "string"},
        {"name": "title", "type": "string"},
        {"name": "content", "type": "string"},
        {"name": "labels", "type": "list", "default": []},
        {"name": "post_id", "type": "string"},
        {"name": "publish", "type": "boolean", "default": True},
    ],
}

def scenario(params):
    import blogger_automation as b
    blog_id = params["blog_id"]
    labels = params.get("labels") or []
    is_draft = not bool(params.get("publish", True))
    if params.get("post_id"):
        post = b.blogger_update_post(blog_id, params["post_id"], params["title"], params.get("content", ""), labels)
    else:
        post = b.blogger_create_post(blog_id, params["title"], params.get("content", ""), labels, is_draft)
    if not is_draft and post.status == "DRAFT":
        post = b.blogger_publish_post(blog_id, post.id)
    return {"post_id": post.id, "status": post.status, "url": post.url}