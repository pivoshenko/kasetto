"use client";

import { useEffect, useState } from "react";

export function ScrollCue() {
  const [hidden, setHidden] = useState(false);

  useEffect(() => {
    const onScroll = () => {
      setHidden(window.scrollY > 40);
    };
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <div className="scroll-cue" data-hidden={hidden} aria-hidden>
      <span className="scroll-cue-label">scroll</span>
      <span className="scroll-cue-chevron" />
    </div>
  );
}
