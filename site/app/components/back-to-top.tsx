"use client";

import { useEffect, useState } from "react";
import { GoArrowUp } from "react-icons/go";

// Shown once the reader is past roughly one screen of content
const THRESHOLD = 640;

export function BackToTop() {
  const [pastFold, setPastFold] = useState(false);
  const [atFooter, setAtFooter] = useState(false);

  useEffect(() => {
    const onScroll = () => setPastFold(window.scrollY > THRESHOLD);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  // The button is fixed to the bottom-right corner, which is where the footer's own content
  // ends up. Stand down while the footer is on screen rather than covering it
  useEffect(() => {
    const footer = document.querySelector("footer");
    if (!footer) return;
    const observer = new IntersectionObserver(([entry]) => setAtFooter(entry.isIntersecting), {
      rootMargin: "0px 0px -24px 0px",
    });
    observer.observe(footer);
    return () => observer.disconnect();
  }, []);

  const visible = pastFold && !atFooter;

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
