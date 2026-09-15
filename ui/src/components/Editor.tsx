import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Underline from "@tiptap/extension-underline";
import Link from "@tiptap/extension-link";
import Image from "@tiptap/extension-image";
import Highlight from "@tiptap/extension-highlight";
import TextAlign from "@tiptap/extension-text-align";
import Placeholder from "@tiptap/extension-placeholder";

import { Toolbar } from "./Toolbar";

export interface EditorHandle {
  html: string;
  wordCount: number;
}

interface Props {
  initial: string;
  wordCount: number;
  uploadEnabled: boolean;
  uploading: boolean;
  onUpload: () => Promise<string | null>;
  onChange: (html: string, wordCount: number) => void;
}

export function Editor({
  initial,
  uploadEnabled,
  uploading,
  onUpload,
  onChange,
}: Props) {
  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        codeBlock: { exitOnArrowDown: true },
        code: {},
      }),
      Underline,
      Highlight.configure({ multicolor: false }),
      Link.configure({ openOnClick: false, autolink: true }),
      Image.configure({ inline: false, allowBase64: false }),
      TextAlign.configure({ types: ["heading", "paragraph"] }),
      Placeholder.configure({ placeholder: "Viết bài viết của bạn tại đây…" }),
    ],
    content: initial,
    editorProps: {
      attributes: {
        class: "bp-prose",
        spellcheck: "false",
      },
    },
    onUpdate: ({ editor }) => {
      onChange(editor.getHTML(), editor.storage.characterCount?.words());
    },
    onCreate: ({ editor }) => {
      onChange(editor.getHTML(), editor.storage.characterCount?.words());
      editor.chain().focus().run();
    },
  });

  if (!editor) return <div className="bp-editor-loading">Đang tải editor…</div>;

  const doUpload = async () => {
    if (uploading || !uploadEnabled) return;
    const url = await onUpload();
    if (url) {
      editor.chain().focus().setImage({ src: url }).run();
    }
  };

  return (
    <div className="bp-editor">
      <Toolbar editor={editor} uploading={uploading} uploadEnabled={uploadEnabled} onUpload={doUpload} />
      <EditorContent editor={editor} />
    </div>
  );
}