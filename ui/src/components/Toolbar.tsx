import type { Editor } from "@tiptap/react";

const Button = ({
  onClick,
  active,
  disabled,
  title,
  children,
}: {
  onClick: () => void;
  active?: boolean;
  disabled?: boolean;
  title?: string;
  children: React.ReactNode;
}) => (
  <button
    type="button"
    className={`bp-tb-btn${active ? " active" : ""}`}
    onMouseDown={(e) => e.preventDefault()}
    onClick={onClick}
    disabled={disabled}
    title={title}
  >
    {children}
  </button>
);

const Separator = () => <span className="bp-tb-sep" />;

export function Toolbar({
  editor,
  uploading,
  uploadEnabled,
  onUpload,
}: {
  editor: Editor;
  uploading: boolean;
  uploadEnabled: boolean;
  onUpload: () => void | Promise<void>;
}) {
  const upload = () => {
    if (uploading) return;
    editor.chain().focus().run();
    const res = onUpload();
    if (res && typeof (res as Promise<void>).then === "function") {
      (res as Promise<void>).catch(() => {});
    }
  };

  return (
    <div className="bp-toolbar">
      <select
        className="bp-tb-select"
        value={
          editor.isActive("paragraph")
            ? "p"
            : editor.isActive("heading", { level: 1 })
              ? "h1"
              : editor.isActive("heading", { level: 2 })
                ? "h2"
                : editor.isActive("heading", { level: 3 })
                  ? "h3"
                  : "p"
        }
        onChange={(e) => {
          const v = e.target.value;
          editor.chain().focus().run();
          if (v === "p") editor.chain().focus().setParagraph().run();
          if (v === "h1") editor.chain().focus().toggleHeading({ level: 1 }).run();
          if (v === "h2") editor.chain().focus().toggleHeading({ level: 2 }).run();
          if (v === "h3") editor.chain().focus().toggleHeading({ level: 3 }).run();
        }}
      >
        <option value="p">Văn bản</option>
        <option value="h1">Tiêu đề 1</option>
        <option value="h2">Tiêu đề 2</option>
        <option value="h3">Tiêu đề 3</option>
      </select>

      <Separator />

      <Button title="In đậm (Ctrl/Cmd+B)" active={editor.isActive("bold")} onClick={() => editor.chain().focus().toggleBold().run()}>
        <b>B</b>
      </Button>
      <Button title="In nghiêng (Ctrl/Cmd+I)" active={editor.isActive("italic")} onClick={() => editor.chain().focus().toggleItalic().run()}>
        <i>I</i>
      </Button>
      <Button title="Gạch chân (Ctrl/Cmd+U)" active={editor.isActive("underline")} onClick={() => editor.chain().focus().toggleUnderline().run()}>
        <u>U</u>
      </Button>
      <Button title="Gạch ngang" active={editor.isActive("strike")} onClick={() => editor.chain().focus().toggleStrike().run()}>
        <s>S</s>
      </Button>
      <Button title="Tô màu" active={editor.isActive("highlight")} onClick={() => editor.chain().focus().toggleHighlight().run()}>
        <span style={{ background: "#ffd54f" }}>ab</span>
      </Button>

      <Separator />

      <Button title="Danh sách chấm" active={editor.isActive("bulletList")} onClick={() => editor.chain().focus().toggleBulletList().run()}>
        •≡
      </Button>
      <Button title="Danh sách số" active={editor.isActive("orderedList")} onClick={() => editor.chain().focus().toggleOrderedList().run()}>
        1≡
      </Button>
      <Button title="Code inline" active={editor.isActive("code")} onClick={() => editor.chain().focus().toggleCode().run()}>
        {"</>"}
      </Button>
      <Button title="Khối code" active={editor.isActive("codeBlock")} onClick={() => editor.chain().focus().toggleCodeBlock().run()}>
        {"⚙"}
      </Button>
      <Button title="Trích dẫn" active={editor.isActive("blockquote")} onClick={() => editor.chain().focus().toggleBlockquote().run()}>
        ❝
      </Button>

      <Separator />

      <Button title="Căn trái" active={editor.isActive({ textAlign: "left" })} onClick={() => editor.chain().focus().setTextAlign("left").run()}>
        ⇤
      </Button>
      <Button title="Căn giữa" active={editor.isActive({ textAlign: "center" })} onClick={() => editor.chain().focus().setTextAlign("center").run()}>
        ⇔
      </Button>
      <Button title="Căn phải" active={editor.isActive({ textAlign: "right" })} onClick={() => editor.chain().focus().setTextAlign("right").run()}>
        ⇥
      </Button>

      <Separator />

      <Button
        title="Upload ảnh (tải qua Chrome, lấy URL Blogger)"
        onClick={upload}
        disabled={!uploadEnabled || uploading}
        active={uploading}
      >
        {uploading ? "⏳ Đang upload…" : "🖼 Ảnh"}
      </Button>
      <Button
        title="Chèn link (Ctrl/Cmd+K)"
        active={editor.isActive("link")}
        onClick={() => {
          const prev = editor.getAttributes("link").href as string | undefined;
          const url = window.prompt("Nhập URL:", prev || "https://");
          if (url === null) return;
          if (url === "") {
            editor.chain().focus().extendMarkRange("link").unsetLink().run();
            return;
          }
          editor.chain().focus().extendMarkRange("link").setLink({ href: url }).run();
        }}
      >
        🔗
      </Button>

      <Separator />

      <Button
        title="Xoá định dạng"
        onClick={() => editor.chain().focus().clearNodes().unsetAllMarks().run()}
      >
        ⌫
      </Button>
      <Button title="Hoàn tác (Ctrl/Cmd+Z)" onClick={() => editor.chain().focus().undo().run()}>
        ⟲
      </Button>
      <Button title="Làm lại (Ctrl/Cmd+Shift+Z)" onClick={() => editor.chain().focus().redo().run()}>
        ⟳
      </Button>

      <span className="bp-tb-spacer" />
    </div>
  );
}