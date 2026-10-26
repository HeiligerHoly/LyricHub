// Render settings for the LyricHub promo (1920x1080, 60 fps from src/data/scenes.ts).
import { Config } from "@remotion/cli/config";

Config.setVideoImageFormat("jpeg");
Config.setJpegQuality(95);
Config.setCodec("h264");
Config.setPixelFormat("yuv420p");
// Standard HD colour (BT.709, limited range) so YouTube and players show the colours as rendered.
Config.setColorSpace("bt709");
Config.setCrf(18);
// Always write an audio track, silent until a voice-over file is dropped into public/vo/.
Config.setEnforceAudioTrack(true);
Config.setOverwriteOutput(true);
