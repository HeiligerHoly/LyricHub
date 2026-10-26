// Places the overlay like LyricHub does (top centre, 2.2 % from the top on 1080p), optionally
// zoomed for close-ups. Zooming is a CSS transform, so the overlay's own values stay real.
import React from "react";
import { Overlay, type OverlayProps } from "./Overlay";

export const OverlayStage: React.FC<OverlayProps & { zoom?: number; top?: number; centerX?: number; o?: number }> = ({
  zoom = 1,
  top = 24,
  centerX = 960,
  o = 1,
  ...props
}) => (
  <div
    style={{
      position: "absolute",
      left: centerX,
      top,
      transform: `translateX(-50%) scale(${zoom})`,
      transformOrigin: "top center",
      opacity: o,
    }}
  >
    <Overlay {...props} />
  </div>
);
