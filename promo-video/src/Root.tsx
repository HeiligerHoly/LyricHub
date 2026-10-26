// Compositions: "Master" (the whole promo) and one composition per scene in the "Scenes" folder.
// All lengths come from src/data/scenes.ts; per-scene voice-over files can stretch a scene, and
// the master adds the background music from public/music/.
import React from "react";
import { AbsoluteFill, Audio, Composition, Folder, Series, staticFile } from "remotion";
import { FPS, HEIGHT, SCENES, WIDTH } from "./data/scenes";
import { S04Overlay, S05OutOfTheWay, S06Hotkeys, S07EditMode, S08Behavior } from "./scenes/Core";
import { S01Hook, S02What, S03Setup } from "./scenes/Intro";
import { S09Layouts, S10Appearance, S11Sync, S12Romaji, S13Settings, S14Tests, S15Outro } from "./scenes/More";
import { BackgroundMusic } from "./music";
import { EMPTY_TIMELINE, sceneDurations, sceneVoFile, singleVoFile, timeline, type Timeline } from "./vo";

export const SCENE_COMPONENTS: Record<string, React.FC> = {
  "s01-hook": S01Hook,
  "s02-what": S02What,
  "s03-setup": S03Setup,
  "s04-overlay": S04Overlay,
  "s05-out-of-the-way": S05OutOfTheWay,
  "s06-hotkeys": S06Hotkeys,
  "s07-edit-mode": S07EditMode,
  "s08-behavior": S08Behavior,
  "s09-layouts": S09Layouts,
  "s10-appearance": S10Appearance,
  "s11-sync": S11Sync,
  "s12-romaji": S12Romaji,
  "s13-settings": S13Settings,
  "s14-tests": S14Tests,
  "s15-outro": S15Outro,
};

const defaultDurations = SCENES.map((s) => Math.round(s.durationSec * FPS));

const SceneWithVo: React.FC<{ id: string }> = ({ id }) => {
  const C = SCENE_COMPONENTS[id];
  const vo = sceneVoFile(id);
  return (
    <AbsoluteFill>
      <C />
      {vo ? <Audio src={staticFile(vo)} /> : null}
    </AbsoluteFill>
  );
};

export const Master: React.FC<Timeline> = (t) => {
  const single = singleVoFile();
  return (
    <AbsoluteFill style={{ background: "#0b0d14" }}>
      <Series>
        {SCENES.map((s, i) => (
          <Series.Sequence key={s.id} name={s.id} durationInFrames={t.durations[i] ?? defaultDurations[i]}>
            <SceneWithVo id={s.id} />
          </Series.Sequence>
        ))}
      </Series>
      {single ? <Audio src={staticFile(single)} /> : null}
      <BackgroundMusic timeline={t} />
    </AbsoluteFill>
  );
};

export const Root: React.FC = () => (
  <>
    <Composition
      id="Master"
      component={Master}
      fps={FPS}
      width={WIDTH}
      height={HEIGHT}
      durationInFrames={defaultDurations.reduce((a, b) => a + b, 0)}
      defaultProps={EMPTY_TIMELINE}
      calculateMetadata={async () => {
        const t = await timeline();
        return { durationInFrames: t.durations.reduce((a, b) => a + b, 0), props: t };
      }}
    />
    <Folder name="Scenes">
      {SCENES.map((s, i) => (
        <Composition
          key={s.id}
          id={s.id}
          component={SceneWithVo}
          fps={FPS}
          width={WIDTH}
          height={HEIGHT}
          durationInFrames={defaultDurations[i]}
          defaultProps={{ id: s.id }}
          calculateMetadata={async () => ({ durationInFrames: (await sceneDurations())[i] })}
        />
      ))}
    </Folder>
  </>
);
