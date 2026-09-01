import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";
import QuickAddWindow from "./lib/components/QuickAddWindow.svelte";

/* One bundle serves both windows. The popup is opened by the tray and the
   global hotkey with ?window=quickadd, which is read here rather than from the
   Tauri window label so the choice is made before anything renders. */
const isQuickAdd = new URLSearchParams(location.search).get("window") === "quickadd";
document.documentElement.dataset.window = isQuickAdd ? "quickadd" : "main";

export default mount(isQuickAdd ? QuickAddWindow : App, {
  target: document.getElementById("app")!,
});
