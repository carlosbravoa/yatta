import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";
import AboutWindow from "./lib/components/AboutWindow.svelte";
import QuickAddWindow from "./lib/components/QuickAddWindow.svelte";

/* One bundle serves every window. The popups are opened with ?window=..., read
   here rather than from the Tauri window label so the choice is made before
   anything renders. */
const which = new URLSearchParams(location.search).get("window") ?? "main";
document.documentElement.dataset.window = which;

const ROOTS = { quickadd: QuickAddWindow, about: AboutWindow, main: App } as const;

export default mount(ROOTS[which as keyof typeof ROOTS] ?? App, {
  target: document.getElementById("app")!,
});
