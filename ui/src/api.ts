import { invoke } from "@tauri-apps/api/core";

export type Settings = {
  chrome_port: number;
  chrome_target_id: string | null;
  editor_mode: string;
  oauth_port: number;
  selected_blog_id: string | null;
  image_timeout_ms: number;
};

export type Blog = {
  id: string;
  name: string;
  description: string;
  url: string;
  status?: string;
  posts?: { total_items: number; is_draft: boolean };
  pages?: { total_items: number; is_draft: boolean };
};

export type Post = {
  id: string;
  title: string;
  content: string;
  status: string;
  published?: string;
  updated?: string;
  url?: string;
  labels: string[];
  is_draft: boolean;
};

export type PostRow = {
  id: string;
  title: string;
  status: string;
  published?: string;
  updated?: string;
  is_draft: boolean;
  labels: string[];
  url?: string;
  snippet: string;
};

export type TagInfo = { name: string; source: string };
export type TabInfo = { id: string; title: string; url: string; target_type?: string };

export type OAuthStatus = {
  configured: boolean;
  has_tokens: boolean;
  expired: boolean;
  has_refresh?: string;
};

export type ChromeStatus = { connected: boolean; tabs: TabInfo[] };
export type ScenarioInfo = { name: string; description: string; file: string; params: unknown[] };

export async function invokeC<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

export const api = {
  getSettings: () => invokeC<Settings>("get_settings"),
  updateSettings: (patch: Record<string, unknown>) =>
    invokeC<Settings>("update_settings", { patch }),

  chromeStatus: () => invokeC<ChromeStatus>("chrome_status"),
  chromeLaunch: (port: number) => invokeC<void>("chrome_launch", { port }),
  chromeConnect: (port: number) => invokeC<void>("chrome_connect", { port }),
  chromeOpenEditor: (blogId: string, draftTitle?: string, draftContent?: string) =>
    invokeC<string>("chrome_open_editor", { blog_id: blogId, draft_title: draftTitle, draft_content: draftContent }),
  chromeCloseTab: (targetId: string) => invokeC<void>("chrome_close_tab", { target_id: targetId }),

  uploadImage: (path: string, targetId?: string | null) =>
    invokeC<string>("upload_image", { path, target_id: targetId }),

  oauthGetConfig: () =>
    invokeC<{ client_id: string; client_secret: string; redirect_uri: string; configured: boolean }>(
      "oauth_get_config",
    ),
  oauthSetConfig: (clientId: string, clientSecret: string, redirectUri?: string) =>
    invokeC<void>("oauth_set_config", { client_id: clientId, client_secret: clientSecret, redirect_uri: redirectUri }),
  oauthLoginStart: () =>
    invokeC<{ url: string; target_id: string | null; manual: boolean }>("oauth_login_start"),
  oauthStatus: () => invokeC<OAuthStatus>("oauth_status"),
  oauthLogout: () => invokeC<void>("oauth_logout"),

  bloggerListBlogs: (refresh: boolean) =>
    invokeC<Blog[]>("blogger_list_blogs", { refresh }),
  bloggerGetBlog: (blogId: string, refresh: boolean) =>
    invokeC<Blog>("blogger_get_blog", { blog_id: blogId, refresh }),
  bloggerTags: (blogId: string, refresh: boolean) =>
    invokeC<TagInfo[]>("blogger_tags", { blog_id: blogId, refresh }),
  bloggerAddTag: (blogId: string, name: string) =>
    invokeC<string[]>("blogger_add_tag", { blog_id: blogId, name }),
  bloggerRemoveTag: (blogId: string, name: string) =>
    invokeC<void>("blogger_remove_tag", { blog_id: blogId, name }),
  bloggerSettings: (blogId: string, refresh: boolean) =>
    invokeC<Blog>("blogger_settings", { blog_id: blogId, refresh }),
  bloggerPosts: (blogId: string, refresh: boolean) =>
    invokeC<PostRow[]>("blogger_posts", { blog_id: blogId, refresh }),
  bloggerGetPost: (blogId: string, postId: string, refresh: boolean) =>
    invokeC<Post>("blogger_get_post", { blog_id: blogId, post_id: postId, refresh }),
  bloggerCreatePost: (
    blogId: string,
    title: string,
    content: string,
    labels: string[],
    isDraft: boolean,
  ) => invokeC<Post>("blogger_create_post", { blog_id: blogId, title, content, labels, is_draft: isDraft }),
  bloggerUpdatePost: (
    blogId: string,
    postId: string,
    title: string,
    content: string,
    labels: string[],
  ) =>
    invokeC<Post>("blogger_update_post", { blog_id: blogId, post_id: postId, title, content, labels }),
  bloggerPublishPost: (blogId: string, postId: string) =>
    invokeC<Post>("blogger_publish_post", { blog_id: blogId, post_id: postId }),
  bloggerRevertPost: (blogId: string, postId: string) =>
    invokeC<Post>("blogger_revert_post", { blog_id: blogId, post_id: postId }),
  bloggerDeletePost: (blogId: string, postId: string) =>
    invokeC<void>("blogger_delete_post", { blog_id: blogId, post_id: postId }),

  scenarioList: () => invokeC<ScenarioInfo[]>("scenarios_list"),
  scenarioRun: (name: string, params: Record<string, unknown>) =>
    invokeC<unknown>("scenario_run", { name, params }),
  pythonSelftest: () => invokeC<string>("python_selftest"),
};