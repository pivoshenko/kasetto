"use client";

import { useState } from "react";
import { IoCheckmarkOutline, IoCopyOutline } from "react-icons/io5";

export function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  function handleCopy() {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  }

  return (
    <button
      type="button"
      onClick={handleCopy}
      className={`copy-btn${copied ? " copied" : ""}`}
      aria-label="Copy to clipboard"
    >
      {copied ? <IoCheckmarkOutline size={15} /> : <IoCopyOutline size={15} />}
    </button>
  );
}
