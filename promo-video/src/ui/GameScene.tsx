// Abstract, generated "game scene": layered silhouettes, a perspective grid, drifting particles
// and generic HUD shapes. Not modelled on any real game.
import React from "react";
import { AbsoluteFill, useCurrentFrame, useVideoConfig } from "remotion";

const rand = (i: number) => {
  const x = Math.sin(i * 127.1 + 311.7) * 43758.5453;
  return x - Math.floor(x);
};

export const GameScene: React.FC<{ hue?: number; hud?: boolean; dim?: number; speed?: number }> = ({
  hue = 0,
  hud = true,
  dim = 0,
  speed = 1,
}) => {
  const frame = useCurrentFrame();
  const { width, height } = useVideoConfig();
  const t = (frame / 60) * speed;
  const horizon = height * 0.58;

  const ridge = (seed: number, amp: number, base: number, shift: number) => {
    const pts: string[] = [];
    for (let x = -200; x <= width + 200; x += 80) {
      const k = (x + shift) / 260;
      const y = base - amp * (0.5 + 0.5 * Math.sin(k + seed)) - amp * 0.35 * Math.sin(k * 2.3 + seed * 3);
      pts.push(`${x},${y.toFixed(1)}`);
    }
    return `M-200,${height} L${pts.join(" L")} L${width + 200},${height} Z`;
  };

  return (
    <AbsoluteFill style={{ filter: `hue-rotate(${hue}deg)` }}>
      <AbsoluteFill style={{ background: "linear-gradient(180deg, #070a18 0%, #141238 45%, #2a1846 60%, #0a0c16 100%)" }} />
      {/* glow behind the horizon */}
      <div
        style={{
          position: "absolute",
          left: width * 0.5 - 520,
          top: horizon - 300,
          width: 1040,
          height: 420,
          borderRadius: "50%",
          background: "radial-gradient(closest-side, rgba(255,140,90,.35), rgba(255,90,140,.12), transparent)",
        }}
      />
      <svg width={width} height={height} style={{ position: "absolute", inset: 0 }}>
        <path d={ridge(1, 140, horizon - 20, t * 10)} fill="#1b1a40" />
        <path d={ridge(4, 100, horizon + 10, t * 28)} fill="#141633" />
        <path d={ridge(7, 70, horizon + 40, t * 55)} fill="#0e1126" />
        {/* perspective grid floor */}
        <g stroke="rgba(120,160,255,.22)" strokeWidth={1.5}>
          {Array.from({ length: 23 }, (_, i) => {
            const x = (i - 11) * 160;
            return <line key={`v${i}`} x1={width / 2 + x * 0.12} y1={horizon + 40} x2={width / 2 + x * 1.6} y2={height} />;
          })}
          {Array.from({ length: 10 }, (_, i) => {
            const z = ((i + (t * 0.8) % 1) / 10) ** 2;
            const y = horizon + 40 + z * (height - horizon - 40);
            return <line key={`h${i}`} x1={0} y1={y} x2={width} y2={y} opacity={0.3 + z} />;
          })}
        </g>
        {/* drifting particles */}
        {Array.from({ length: 40 }, (_, i) => {
          const x = (rand(i) * width + t * (10 + rand(i + 9) * 30)) % width;
          const y = (rand(i + 3) * horizon * 0.9 + Math.sin(t + i) * 6) % horizon;
          const r = 1 + rand(i + 5) * 2.2;
          return <circle key={i} cx={x} cy={y} r={r} fill="rgba(200,220,255,.5)" />;
        })}
      </svg>
      {hud ? (
        <>
          {/* generic crosshair */}
          <div style={{ position: "absolute", left: width / 2 - 14, top: height / 2 - 1, width: 28, height: 2, background: "rgba(255,255,255,.55)" }} />
          <div style={{ position: "absolute", left: width / 2 - 1, top: height / 2 - 14, width: 2, height: 28, background: "rgba(255,255,255,.55)" }} />
          {/* generic health-style bar */}
          <div style={{ position: "absolute", left: 60, bottom: 230, width: 300, height: 14, borderRadius: 7, background: "rgba(255,255,255,.12)" }}>
            <div style={{ width: "72%", height: "100%", borderRadius: 7, background: "rgba(120,230,170,.75)" }} />
          </div>
          {/* generic map-style circle */}
          <div style={{ position: "absolute", right: 60, top: 60, width: 150, height: 150, borderRadius: "50%", border: "2px solid rgba(255,255,255,.25)", background: "rgba(10,14,30,.45)" }}>
            <div style={{ position: "absolute", left: 69, top: 69, width: 12, height: 12, borderRadius: 3, background: "rgba(255,211,110,.85)", transform: `rotate(${t * 20}deg)` }} />
          </div>
        </>
      ) : null}
      {dim ? <AbsoluteFill style={{ background: `rgba(0,0,0,${dim})` }} /> : null}
    </AbsoluteFill>
  );
};
