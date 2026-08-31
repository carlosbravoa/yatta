/** Deterministic tag colours.
 *
 *  A tag keeps the same hue everywhere and across restarts because the hue is
 *  derived from the tag name itself — no palette to store, no colours to
 *  assign. Lightness is the only thing the theme changes, so contrast holds in
 *  both light and dark.
 */

function hashHue(name: string): number {
  let h = 0;
  for (let i = 0; i < name.length; i++) {
    h = (h * 31 + name.charCodeAt(i)) >>> 0;
  }
  // Skip the muddy yellow-green band (70-95deg) where text contrast suffers.
  const hue = h % 335;
  return hue >= 70 ? hue + 25 : hue;
}

export function tagStyle(name: string): string {
  const hue = hashHue(name);
  return `--tag-h:${hue};`;
}

export function tagHue(name: string): number {
  return hashHue(name);
}
