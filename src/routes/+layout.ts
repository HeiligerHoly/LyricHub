// Tauri has no Node.js server, so the app runs as a SPA (adapter-static with index.html fallback).
// Both windows load this app: "/" is the settings window, "/overlay" the overlay window.
export const ssr = false;
