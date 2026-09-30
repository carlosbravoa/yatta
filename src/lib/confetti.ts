/** A proper party for finishing a task: a burst from the task plus two corner
 *  cannons. No dependency: one
 *  throwaway canvas over the window, removed as soon as the last piece lands. */

const TOKENS = ["--accent", "--accent-2", "--p-urgent", "--p-high", "--p-medium", "--p-low", "--today"];
const BURST = 110;
const CANNON = 70;
const GRAVITY = 0.28;
const DRAG = 0.982;
const LIFETIME = 2600;
const FADE = 700;

interface Piece {
  x: number;
  y: number;
  vx: number;
  vy: number;
  angle: number;
  spin: number;
  w: number;
  h: number;
  shape: "rect" | "circle" | "streamer";
  wobble: number;
  color: string;
}

/** Burst from the task's checkbox, or the card itself on the board. Finds it
 *  by path so callers only need the task, not a DOM element. */
export function celebrate(path: string) {
  const row = document.querySelector(`[data-task][data-path="${CSS.escape(path)}"]`);
  const origin = row?.querySelector(".check") ?? row;
  confetti(origin);
}

export function confetti(origin: Element | null) {
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) return;

  const rect = origin?.getBoundingClientRect();
  const cx = rect ? rect.left + rect.width / 2 : innerWidth / 2;
  const cy = rect ? rect.top + rect.height / 2 : innerHeight / 2;

  // Theme colours, so the burst matches light and dark mode alike.
  const style = getComputedStyle(document.documentElement);
  const colors = TOKENS.map((t) => style.getPropertyValue(t).trim()).filter(Boolean);
  if (!colors.length) colors.push("#6366f1");

  const canvas = document.createElement("canvas");
  const dpr = devicePixelRatio || 1;
  canvas.width = innerWidth * dpr;
  canvas.height = innerHeight * dpr;
  canvas.setAttribute("aria-hidden", "true");
  Object.assign(canvas.style, {
    position: "fixed",
    inset: "0",
    width: "100%",
    height: "100%",
    pointerEvents: "none",
    zIndex: "9999",
  });
  document.body.appendChild(canvas);
  const ctx = canvas.getContext("2d");
  if (!ctx) return canvas.remove();
  ctx.scale(dpr, dpr);

  const piece = (x: number, y: number, angle: number, speed: number): Piece => {
    const roll = Math.random();
    const shape = roll < 0.55 ? "rect" : roll < 0.8 ? "circle" : "streamer";
    return {
      x,
      y,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      angle: Math.random() * Math.PI,
      spin: (Math.random() - 0.5) * 0.5,
      w: shape === "streamer" ? 4 : 8 + Math.random() * 6,
      h: shape === "streamer" ? 16 + Math.random() * 10 : 5 + Math.random() * 5,
      shape,
      wobble: Math.random() * Math.PI * 2,
      color: colors[Math.floor(Math.random() * colors.length)],
    };
  };

  const pieces: Piece[] = [];
  // The main burst: a wide, mostly upward fan from the task itself.
  for (let i = 0; i < BURST; i++) {
    const angle = -Math.PI / 2 + (Math.random() - 0.5) * Math.PI * 1.3;
    pieces.push(piece(cx, cy, angle, 7 + Math.random() * 11));
  }
  // Two cannons from the bottom corners, aimed up and inwards.
  for (let i = 0; i < CANNON; i++) {
    const spread = (Math.random() - 0.5) * 0.5;
    const speed = 16 + Math.random() * 12;
    pieces.push(piece(0, innerHeight, -Math.PI / 3 + spread, speed));
    pieces.push(piece(innerWidth, innerHeight, (-2 * Math.PI) / 3 + spread, speed));
  }

  const start = performance.now();
  function frame(now: number) {
    const elapsed = now - start;
    ctx!.clearRect(0, 0, innerWidth, innerHeight);
    // Full strength while it rains, then a quick fade at the end.
    ctx!.globalAlpha = Math.min(1, Math.max(0, (LIFETIME - elapsed) / FADE));
    for (const p of pieces) {
      p.vx *= DRAG;
      p.vy = p.vy * DRAG + GRAVITY;
      p.wobble += 0.1;
      // Side-to-side sway as pieces fall, like paper catching the air.
      p.x += p.vx + Math.sin(p.wobble) * 0.8;
      p.y += p.vy;
      p.angle += p.spin;
      if (p.y > innerHeight + 40) continue;
      ctx!.save();
      ctx!.translate(p.x, p.y);
      ctx!.rotate(p.angle);
      // Squash on one axis so pieces look like they flutter.
      ctx!.scale(1, Math.cos(p.angle * 2));
      ctx!.fillStyle = p.color;
      if (p.shape === "circle") {
        ctx!.beginPath();
        ctx!.arc(0, 0, p.w / 2, 0, Math.PI * 2);
        ctx!.fill();
      } else {
        ctx!.fillRect(-p.w / 2, -p.h / 2, p.w, p.h);
      }
      ctx!.restore();
    }
    if (elapsed < LIFETIME) requestAnimationFrame(frame);
    else canvas.remove();
  }
  requestAnimationFrame(frame);
}
