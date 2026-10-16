"use client";

import { useEffect, useRef, useState } from "react";
import { IoCheckmarkOutline, IoCopyOutline } from "react-icons/io5";

// The async Clipboard API is refused in some contexts (no focus, embedded frames, older
// Safari), so fall back to the selection-based copy instead of failing silently
async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand("copy");
    area.remove();
    return ok;
  }
}

export function CopyButton({ text, label }: { text: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  async function handleCopy() {
    if (!(await copyText(text))) return;
    setCopied(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied(false), 1500);
  }

  return (
    <button
      type="button"
      onClick={handleCopy}
      className={`copy-btn${copied ? " copied" : ""}`}
      aria-label={label ?? "Copy to clipboard"}
    >
      {copied ? <IoCheckmarkOutline size={15} /> : <IoCopyOutline size={15} />}
      {/* The label never changes: swapping it for "Copied" resizes the button and shifts the row */}
      {label ? <span>{label}</span> : null}
      <span className="copy-btn-status" role="status">
        {copied ? "Copied" : ""}
      </span>
    </button>
  );
}
