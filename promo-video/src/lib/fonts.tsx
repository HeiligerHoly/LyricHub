// Loads the bundled Inter Variable (the same package LyricHub ships) before anything is drawn,
// so text measuring for the karaoke fill and every rendered frame use the right font.
import "@fontsource-variable/inter";
import React, { useEffect, useState } from "react";
import { continueRender, delayRender } from "remotion";

const WEIGHTS = ["400", "450", "500", "600", "650", "700", "800"];

export const FontGate: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [handle] = useState(() => delayRender("Loading fonts"));
  const [ready, setReady] = useState(false);
  useEffect(() => {
    const loads = WEIGHTS.map((w) => document.fonts.load(`${w} 28px "Inter Variable"`));
    loads.push(document.fonts.load('500 28px "Yu Gothic UI"'));
    Promise.allSettled(loads).then(() => {
      setReady(true);
      continueRender(handle);
    });
  }, [handle]);
  return ready ? <>{children}</> : null;
};
