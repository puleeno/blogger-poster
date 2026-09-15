import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type OAuthStatus, type Blog, type PostRow, type Settings, type TagInfo } from "./api";
import { Editor } from "./components/Editor";
import { StatusBar } from "./components/StatusBar";
import { SettingsModal } from "./components/SettingsModal";

type Toast = { kind: "ok" | "err" | "info"; msg: string };

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [blogs, setBlogs] = useState<Blog[]>([]);
  const [blogId, setBlogId] = useState("");
  const [tags, setTags] = useState<TagInfo[]>([]);
  const [posts, setPosts] = useState<PostRow[]>([]);
  const [chromeConnected, setChromeConnected] = useState(false);
  const [oauth, setOauth] = useState<OAuthStatus>();
  const [scenariosCount, setScenariosCount] = useState(0);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [tagInput, setTagInput] = useState("");
  const [toast, setToast] = useState<Toast | null>(null);
  const [newTag, setNewTag] = useState("");
  const [editorKey, setEditorKey] = useState(0);
  const toastTimer = useRef<number | undefined>(undefined);

  const [post, setPost] = useState({
    id: "",
    title: "",
    content: "",
    labels: [] as string[],
  });
  const [wordCount, setWordCount] = useState(0);

  const show = (kind: Toast["kind"], msg: string) => {
    setToast({ kind, msg });
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 5000);
  };

  // initial load
  useEffect(() => {
    api.getSettings().then((s) => {
      setSettings(s);
      if (s.selected_blog_id) setBlogId(s.selected_blog_id);
    });
    api.oauthStatus().then(setOauth).catch(() => {});
    api.scenarioList().then((l) => setScenariosCount(l.length)).catch(() => {});
    api.bloggerListBlogs(true).then((bs) => {
      setBlogs(bs);
      if (bs.length) {
        const sel = settings?.selected_blog_id && bs.find((b) => b.id === settings.selected_blog_id);
        const picked = sel ? sel.id : bs[0].id;
        setBlogId(picked);
        void api.updateSettings({ selected_blog_id: picked });
      }
    }).catch((e) => show("err", String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // poll chrome + oauth status
  useEffect(() => {
    const poll = setInterval(async () => {
      try {
        const st = await api.chromeStatus();
        setChromeConnected(st.connected);
        if (st.connected) {
          const t = st.tabs.find((x) => x.url.includes("blogger.com/blog/post/edit"));
          const pending = await api.getSettings();
          if (t && pending.chrome_target_id !== t.id) {
            void api.updateSettings({ chrome_target_id: t.id });
            if (pending.chrome_target_id) void api.chromeCloseTab(pending.chrome_target_id);
          }
        }
      } catch {}
      try {
        setOauth(await api.oauthStatus());
      } catch {}
    }, 4000);
    return () => clearInterval(poll);
  }, []);

  // load collection when blog changes
  const loadBlogData = useCallback(
    async (bid: string, { refreshTags, refreshPosts }: { refreshTags?: boolean; refreshPosts?: boolean } = {}) => {
      if (!bid) return;
      try {
        const [t, p] = await Promise.all([
          api.bloggerTags(bid, !!refreshTags),
          api.bloggerPosts(bid, !!refreshPosts),
        ]);
        setTags(t);
        setPosts(p);
      } catch (e) {
        show("err", String(e));
      }
    },
    [],
  );

  useEffect(() => {
    if (!blogId) return;
    void loadBlogData(blogId);
  }, [blogId, loadBlogData]);

  const onSelectBlog = (bid: string) => {
    setBlogId(bid);
    setPost({ id: "", title: "", content: "", labels: [] });
    setEditorKey((k) => k + 1);
    void api.updateSettings({ selected_blog_id: bid });
  };

  const uploadImage = async (): Promise<string | null> => {
    if (!blogId) {
      show("err", "Chọn blog trước khi upload ảnh");
      return null;
    }
    const picked = await open({
      multiple: false,
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg"],
        },
      ],
    });
    if (!picked || Array.isArray(picked)) return null;

    setUploading(true);
    try {
      let targetId = (await api.getSettings()).chrome_target_id;
      if (!chromeConnected) {
        show("err", "Chưa kết nối Chrome. Bật Chrome kèm remote-debugging-port trước.");
        return null;
      }
      if (!targetId) {
        show("info", "Đang mở editor Blogger trong Chrome…");
        targetId = await api.chromeOpenEditor(blogId);
        await api.updateSettings({ chrome_target_id: targetId });
      }
      show("info", "Đang upload ảnh qua Chrome…");
      const url = await api.uploadImage(picked as string, targetId);
      show("ok", "Đã upload ảnh, đã chèn URL vào bài viết.");
      return url;
    } catch (e) {
      show("err", String(e));
      return null;
    } finally {
      setUploading(false);
    }
  };

  const ensureEditorTab = async (): Promise<string> => {
    let targetId = (await api.getSettings()).chrome_target_id;
    if (!chromeConnected) throw new Error("Chưa kết nối Chrome");
    if (!targetId) {
      targetId = await api.chromeOpenEditor(blogId, post.title || undefined);
      await api.updateSettings({ chrome_target_id: targetId });
    }
    return targetId;
  };

  const saveDraft = async (publish: boolean) => {
    if (!blogId) return show("err", "Chưa chọn blog");
    if (!post.title.trim()) return show("err", "Tiêu đề trống");
    setBusy(true);
    try {
      if (post.id) {
        const updated = await api.bloggerUpdatePost(blogId, post.id, post.title, post.content, post.labels);
        if (publish && updated.status === "DRAFT") {
          await api.bloggerPublishPost(blogId, post.id);
        }
      } else {
        await api.bloggerCreatePost(blogId, post.title, post.content, post.labels, !publish);
      }
      show("ok", publish ? "Đã xuất bản bài viết." : "Đã lưu bản nháp.");
      void loadBlogData(blogId, { refreshPosts: true });
    } catch (e) {
      show("err", String(e));
    } finally {
      setBusy(false);
    }
  };

  const openInBlogger = async () => {
    try {
      const targetId = await ensureEditorTab();
      if (post.id || post.title || post.content) {
        void api.chromeOpenEditor(blogId, post.title || undefined, post.content || undefined);
      }
      show("ok", `Mở editor tại Chrome (tab: ${targetId.slice(0, 6)}…)`);
    } catch (e) {
      show("err", String(e));
    }
  };

  const addTag = async () => {
    const name = newTag.trim();
    if (!name || !blogId) return;
    try {
      const list = await api.bloggerAddTag(blogId, name);
      const withSrc = list.map((n) => ({ name: n, source: n.toLowerCase() === name.toLowerCase() ? "local" : "blogger" }));
      setTags(withSrc);
      if (!post.labels.includes(name)) {
        setPost((p) => ({ ...p, labels: [...p.labels, name] }));
      }
      setNewTag("");
      show("ok", `Thêm tag "${name}" (có trong cache local).`);
    } catch (e) {
      show("err", String(e));
    }
  };

  const selectPost = async (row: PostRow) => {
    try {
      const full = await api.bloggerGetPost(blogId, row.id, false);
      setPost({ id: full.id, title: full.title, content: full.content, labels: full.labels });
      setWordCount((full.content.match(/[\p{L}\p{N}\s]/gu) || []).length);
      setEditorKey((k) => k + 1);
      show("info", "Đã nạp bài viết vào editor.");
    } catch (e) {
      show("err", String(e));
    }
  };

  const refreshAll = async () => {
    await api.bloggerListBlogs(true);
    await loadBlogData(blogId, { refreshTags: true, refreshPosts: true });
  };

  return (
    <div className="bp-app">
      <header className="bp-header">
        <div className="bp-brand">
          <span className="bp-logo">B</span> Blogger Poster
        </div>
        <select
          className="bp-blog-select"
          value={blogId}
          onChange={(e) => onSelectBlog(e.target.value)}
        >
          {blogs.length === 0 && <option value="">— chưa có blog —</option>}
          {blogs.map((b) => (
            <option key={b.id} value={b.id}>
              {b.name}
            </option>
          ))}
        </select>

        <div className="bp-header-actions">
          <button className="bp-btn" onClick={() => void openInBlogger()} title="Mở bài viết trên Blogger trong Chrome">
            Mở trên Blogger
          </button>
          <button className="bp-btn" onClick={() => void saveDraft(false)} disabled={busy}>
            {post.id ? "Lưu thay đổi" : "Lưu nháp"}
          </button>
          <button className="bp-btn primary" onClick={() => void saveDraft(true)} disabled={busy}>
            {post.id ? "Xuất bản" : "Tạo bài viết"}
          </button>
        </div>

        {blogId && post.id && (
          <div className="bp-poststatus">
            <button
              className="bp-btn small"
              onClick={() => {
                setPost({ id: "", title: "", content: "", labels: [] });
                setEditorKey((k) => k + 1);
              }}
            >
              ✚ Bài mới
            </button>
          </div>
        )}
      </header>

      <div className="bp-body">
        <aside className="bp-sidebar">
          <section className="bp-panel">
            <header className="bp-panel-head">
              <strong>Tags / Nhãn</strong>
              <span className="bp-panel-actions">
                <button className="bp-link-btn" onClick={() => void loadBlogData(blogId, { refreshTags: true })}>
                  ↻ Sync
                </button>
              </span>
            </header>
            <div className="bp-tags">
              {tags.map((t) => (
                <span key={t.name} className={`bp-tag ${t.source === "local" ? "local" : ""}`}>
                  {t.name}
                  <button
                    className="bp-tag-x"
                    onClick={async () => {
                      await api.bloggerRemoveTag(blogId, t.name).catch(() => {});
                      setTags((prev) => prev.filter((x) => x.name !== t.name));
                    }}
                  >
                    ✕
                  </button>
                </span>
              ))}
              {tags.length === 0 && <span className="bp-empty">Chưa có tag. Bấm Sync để lấy từ Blogger.</span>}
            </div>
            <div className="bp-tag-add">
              <input
                value={newTag}
                onChange={(e) => setNewTag(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && void addTag()}
                placeholder="Thêm tag mới (Enter)"
              />
              <button className="bp-btn small" onClick={() => void addTag()}>
                ＋
              </button>
            </div>
            <p className="bp-hint">
              Tag thêm ở đây sẽ tự lưu vào cache local và không bị mất dù Blogger chưa có.
            </p>
          </section>

          <section className="bp-panel posts">
            <header className="bp-panel-head">
              <strong>Bài viết ({posts.filter((p) => p.status === "LIVE").length} live)</strong>
              <span className="bp-panel-actions">
                <button className="bp-link-btn" onClick={() => void loadBlogData(blogId, { refreshPosts: true })}>
                  ↻ Sync
                </button>
              </span>
            </header>
            <div className="bp-posts">
              {posts.map((row) => (
                <div key={row.id} className={`bp-post-row ${post.id === row.id ? "active" : ""}`} onClick={() => void selectPost(row)}>
                  <div className="bp-post-title">{row.title || "(không tiêu đề)"}</div>
                  <div className="bp-post-meta">
                    <span className={`bp-st ${row.status}`}>{row.status}</span>
                    <span>{row.published?.slice(0, 10) ?? "—"}</span>
                  </div>
                  <div className="bp-post-snippet">{row.snippet}</div>
                  {row.labels.length > 0 && (
                    <div className="bp-post-labels">{row.labels.join(", ")}</div>
                  )}
                </div>
              ))}
              {posts.length === 0 && <span className="bp-empty">Chưa có bài trong cache.</span>}
            </div>
          </section>
        </aside>

        <main className="bp-main">
          <input
            className="bp-title-input"
            placeholder="Tiêu đề bài viết…"
            value={post.title}
            onChange={(e) => setPost((p) => ({ ...p, title: e.target.value }))}
          />

          <div className="bp-tagline">
            <div className="bp-taginput">
              {post.labels.map((l) => (
                <span key={l} className="bp-tag">
                  {l}
                  <button
                    className="bp-tag-x"
                    onClick={() =>
                      setPost((p) => ({ ...p, labels: p.labels.filter((x) => x !== l) }))
                    }
                  >
                    ✕
                  </button>
                </span>
              ))}
              <input
                className="bp-inline-tag"
                value={tagInput}
                onChange={(e) => setTagInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && tagInput.trim()) {
                    const v = tagInput.trim();
                    setPost((p) => ({ ...p, labels: p.labels.includes(v) ? p.labels : [...p.labels, v] }));
                    setTagInput("");
                  }
                }}
                placeholder="Nhãn cho bài này (Enter)…"
              />
            </div>
            <span className="bp-wordcount">{wordCount || 0} từ</span>
          </div>

          <Editor
            key={editorKey}
            initial={post.content}
            wordCount={wordCount}
            uploadEnabled={!!blogId}
            uploading={uploading}
            onUpload={uploadImage}
            onChange={(html) => {
              setPost((p) => ({ ...p, content: html }));
              setWordCount((html.replace(/<[^>]+>/g, " ").match(/\S+/g) || []).length);
            }}
          />

          <div className="bp-actions">
            <button className="bp-btn" onClick={() => void openInBlogger()}>
              Mở editor Blogger
            </button>
            <button className="bp-btn" onClick={uploadImage} disabled={uploading || !blogId}>
              {uploading ? "Đang upload ảnh…" : "Upload ảnh"}
            </button>
            <span className="bp-tb-spacer" />
            <button className="bp-btn" disabled={busy} onClick={() => void saveDraft(false)}>
                Lưu nháp
            </button>
            <button className="bp-btn primary" disabled={busy} onClick={() => void saveDraft(true)}>
              {post.id ? "Xuất bản thay đổi" : "Tạo & xuất bản"}
            </button>
          </div>
        </main>
      </div>

      <StatusBar
        chromeConnected={chromeConnected}
        port={settings?.chrome_port ?? 9222}
        oauth={oauth}
        scenariosCount={scenariosCount}
        onOpenSettings={() => setSettingsOpen(true)}
      />

      {toast && <div className={`bp-toast ${toast.kind}`}>{toast.msg}</div>}

      <SettingsModal
        open={settingsOpen}
        settings={settings}
        onClose={() => setSettingsOpen(false)}
        onSettings={(patch) => {
          const merged = { ...(settings as Settings), ...patch };
          setSettings(merged);
          void api.updateSettings({ ...patch });
        }}
        refreshAll={refreshAll}
      />
    </div>
  );
}