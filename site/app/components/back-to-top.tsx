"use client";

import { useEffect, useState } from "react";
import { GoArrowUp } from "react-icons/go";

// Shown once the reader is past roughly one screen of content
const THRESHOLD = 640;

export function BackToTop() {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const onScroll = () => setVisible(window.scrollY > THRESHOLD);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  function toTop() {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    window.scrollTo({ top: 0, behavior: reduce ? "auto" : "smooth" });
    // Keyboard users land at the start of the page instead of on a button that just hid itself
    document.getElementById("main")?.focus({ preventScroll: true });
  }

  return (
    <button
      type="button"
      className="back-to-top"
      data-visible={visible}
      onClick={toTop}
      aria-label="Back to top"
    >
      <GoArrowUp aria-hidden />
    </button>
  );
}
