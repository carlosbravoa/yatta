/** Startup timing marks, mirrored into the Rust stderr stream so both
 *  timelines read as one sequence. No-ops unless YATTA_TIMING is set, which
 *  the backend decides -- the webview cannot see the environment. */
import { invoke } from "@tauri-apps/api/core";

export function mark(label: string): void {
  invoke("timing_mark", { label }).catch(() => {});
}
