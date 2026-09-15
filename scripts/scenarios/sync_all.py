"""Sync toàn bộ cache: blogs, tags, posts, settings blog từ Blogger API."""
META = {
    "name": "sync_all",
    "description": "Refresh blogs, tags và danh sách bài viết của blog được chọn vào cache SQLite.",
    "params": [{"name": "blog_id", "type": "string"}],
}

def scenario(params):
    import blogger_automation as b
    blogs = b.blogger_list_blogs(True)
    if "blog_id" in params:
        blog_id = params["blog_id"]
    else:
        blog_id = blogs[0].id
    tags = b.blogger_tags(blog_id, True)
    posts = b.blogger_list_posts(blog_id, True)
    return {
        "blogs": len(blogs),
        "tags": tags,
        "posts": len(posts),
        "drafts": sum(1 for p in posts if p.is_draft),
    }