// 3D-просмотр скина без WebGL (раздел 5.9 ТЗ). Окно лаунчера работает без
// видеокарты (бюджет памяти 150 МБ), а WebGL — и skinview3d — добавлял от
// 35 до 100 МБ и не отдавал их после ухода из раздела (замер 01.11.2026).
// Модель игрока — коробки: каждая грань — прямоугольник текстуры, который
// рисуется на холсте 2D аффинным преобразованием, грани — от дальних к
// ближним. Анимации стояния, ходьбы, бега и полёта — как в игре.

export type Pose = "idle" | "walk" | "run" | "fly";

type V3 = [number, number, number];
type Face = {
  /// Углы грани в 3D: верх-лево, верх-право, низ-лево (по текстуре).
  p: [V3, V3, V3];
  normal: V3;
  /// Прямоугольник текстуры.
  uv: [number, number, number, number];
  tex: "skin" | "cape";
};

/// Шесть граней коробки по развёртке Minecraft: (u, v) — угол развёртки,
/// w×h×d — размер в пикселях текстуры, min/max — координаты в модели
/// (игрок смотрит на +z, его правая рука — со стороны −x).
function box(
  u: number,
  v: number,
  w: number,
  h: number,
  d: number,
  min: V3,
  max: V3,
  tex: "skin" | "cape" = "skin",
  mirror = false,
): Face[] {
  const [x0, y0, z0] = min;
  const [x1, y1, z1] = max;
  // В зеркальной развёртке (левые рука и нога старых скинов) стороны меняются.
  const L = mirror ? x1 : x0;
  const R = mirror ? x0 : x1;
  const right: Face = { p: [[x0, y1, z0], [x0, y1, z1], [x0, y0, z0]], normal: [-1, 0, 0], uv: [u, v + d, d, h], tex };
  const left: Face = { p: [[x1, y1, z1], [x1, y1, z0], [x1, y0, z1]], normal: [1, 0, 0], uv: [u + d + w, v + d, d, h], tex };
  const faces: Face[] = [
    { p: [[L, y1, z0], [R, y1, z0], [L, y1, z1]], normal: [0, 1, 0], uv: [u + d, v, w, d], tex },
    { p: [[L, y0, z1], [R, y0, z1], [L, y0, z0]], normal: [0, -1, 0], uv: [u + d + w, v, w, d], tex },
    { p: [[L, y1, z1], [R, y1, z1], [L, y0, z1]], normal: [0, 0, 1], uv: [u + d, v + d, w, h], tex },
    { p: [[R, y1, z0], [L, y1, z0], [R, y0, z0]], normal: [0, 0, -1], uv: [u + 2 * d + w, v + d, w, h], tex },
  ];
  if (mirror) {
    faces.push(
      { p: [[x1, y1, z1], [x1, y1, z0], [x1, y0, z1]], normal: [1, 0, 0], uv: [u, v + d, d, h], tex },
      { p: [[x0, y1, z0], [x0, y1, z1], [x0, y0, z0]], normal: [-1, 0, 0], uv: [u + d + w, v + d, d, h], tex },
    );
  } else faces.push(right, left);
  return faces;
}

/// Коробка, раздутая на `g` во все стороны (внешний слой скина).
function grow(min: V3, max: V3, g: number): [V3, V3] {
  return [
    [min[0] - g, min[1] - g, min[2] - g],
    [max[0] + g, max[1] + g, max[2] + g],
  ];
}

type Part = { faces: Face[]; pivot: V3 };

function parts(slim: boolean, modern: boolean): Record<string, Part> {
  const a = slim ? 3 : 4;
  const head: [V3, V3] = [[-4, 24, -4], [4, 32, 4]];
  const body: [V3, V3] = [[-4, 12, -2], [4, 24, 2]];
  const rArm: [V3, V3] = [[-4 - a, 12, -2], [-4, 24, 2]];
  const lArm: [V3, V3] = [[4, 12, -2], [4 + a, 24, 2]];
  const rLeg: [V3, V3] = [[-4, 0, -2], [0, 12, 2]];
  const lLeg: [V3, V3] = [[0, 0, -2], [4, 12, 2]];
  const out: Record<string, Part> = {
    head: { pivot: [0, 24, 0], faces: [...box(0, 0, 8, 8, 8, ...head), ...box(32, 0, 8, 8, 8, ...grow(...head, 0.5))] },
    body: { pivot: [0, 24, 0], faces: box(16, 16, 8, 12, 4, ...body) },
    rArm: { pivot: [-5, 22, 0], faces: box(40, 16, a, 12, 4, ...rArm) },
    rLeg: { pivot: [-2, 12, 0], faces: box(0, 16, 4, 12, 4, ...rLeg) },
    lArm: { pivot: [5, 22, 0], faces: modern ? box(32, 48, a, 12, 4, ...lArm) : box(40, 16, a, 12, 4, ...lArm, "skin", true) },
    lLeg: { pivot: [2, 12, 0], faces: modern ? box(16, 48, 4, 12, 4, ...lLeg) : box(0, 16, 4, 12, 4, ...lLeg, "skin", true) },
  };
  if (modern) {
    out.body.faces.push(...box(16, 32, 8, 12, 4, ...grow(...body, 0.25)));
    out.rArm.faces.push(...box(40, 32, a, 12, 4, ...grow(...rArm, 0.25)));
    out.lArm.faces.push(...box(48, 48, a, 12, 4, ...grow(...lArm, 0.25)));
    out.rLeg.faces.push(...box(0, 32, 4, 12, 4, ...grow(...rLeg, 0.25)));
    out.lLeg.faces.push(...box(0, 48, 4, 12, 4, ...grow(...lLeg, 0.25)));
  }
  return out;
}

/// Плащ 10×16×1 за спиной; снаружи (к −z) — лицевая сторона развёртки.
function capePart(): Part {
  const min: V3 = [-5, 8, -3];
  const max: V3 = [5, 24, -2];
  const f = box(0, 0, 10, 16, 1, min, max, "cape");
  // В развёртке плаща «перед» (1,1) — наружная сторона: поменять местами
  // переднюю и заднюю грани.
  const front = f.find((x) => x.normal[2] === 1)!;
  const back = f.find((x) => x.normal[2] === -1)!;
  [front.uv, back.uv] = [back.uv, front.uv];
  return { pivot: [0, 24, -2], faces: f };
}

// ── математика ──
const rotX = (p: V3, a: number): V3 => [p[0], p[1] * Math.cos(a) - p[2] * Math.sin(a), p[1] * Math.sin(a) + p[2] * Math.cos(a)];
const rotY = (p: V3, a: number): V3 => [p[0] * Math.cos(a) + p[2] * Math.sin(a), p[1], -p[0] * Math.sin(a) + p[2] * Math.cos(a)];
const rotZ = (p: V3, a: number): V3 => [p[0] * Math.cos(a) - p[1] * Math.sin(a), p[0] * Math.sin(a) + p[1] * Math.cos(a), p[2]];
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];

/// Поворот части вокруг своей оси (x — взмах вперёд-назад, z — в стороны).
type Angles = { x: number; y?: number; z?: number };

/// Яркость граней: свет сверху-спереди; 6 ступеней — заранее затемнённые
/// копии текстуры (затемнять каждую грань при рисовании дороже).
const SHADES = [0.55, 0.65, 0.75, 0.85, 0.93, 1];
const LIGHT: V3 = (() => {
  const v: V3 = [0.35, 0.8, 0.5];
  const n = Math.hypot(...v);
  return [v[0] / n, v[1] / n, v[2] / n];
})();

function shaded(img: ImageData): HTMLCanvasElement[] {
  return SHADES.map((k) => {
    const c = document.createElement("canvas");
    c.width = img.width;
    c.height = img.height;
    const d = new ImageData(new Uint8ClampedArray(img.data), img.width, img.height);
    for (let i = 0; i < d.data.length; i += 4) {
      d.data[i] *= k;
      d.data[i + 1] *= k;
      d.data[i + 2] *= k;
    }
    c.getContext("2d")!.putImageData(d, 0, 0);
    return c;
  });
}

export class Skin3D {
  private ctx: CanvasRenderingContext2D;
  private skin: HTMLCanvasElement[] | null = null;
  private cape: HTMLCanvasElement[] | null = null;
  /// Плащ HD: множитель развёртки (64×32 → 1).
  private capeScale = 1;
  private model: Record<string, Part> = {};
  private capeModel = capePart();
  private pose: Pose = "walk";
  private yaw = 0.45;
  private pitch = 0.12;
  private zoom = 1;
  private frame = 0;
  private start = performance.now();
  private drag: { x: number; y: number; yaw: number; pitch: number } | null = null;
  private off: (() => void)[] = [];

  constructor(private canvas: HTMLCanvasElement) {
    this.ctx = canvas.getContext("2d")!;
    const down = (e: PointerEvent) => {
      this.drag = { x: e.clientX, y: e.clientY, yaw: this.yaw, pitch: this.pitch };
      canvas.setPointerCapture(e.pointerId);
    };
    const move = (e: PointerEvent) => {
      if (!this.drag) return;
      this.yaw = this.drag.yaw + (e.clientX - this.drag.x) * 0.012;
      this.pitch = Math.max(-0.9, Math.min(0.9, this.drag.pitch + (e.clientY - this.drag.y) * 0.008));
    };
    const up = () => (this.drag = null);
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      this.zoom = Math.max(0.6, Math.min(2.2, this.zoom * (e.deltaY < 0 ? 1.1 : 0.9)));
    };
    canvas.addEventListener("pointerdown", down);
    canvas.addEventListener("pointermove", move);
    canvas.addEventListener("pointerup", up);
    canvas.addEventListener("pointercancel", up);
    canvas.addEventListener("wheel", wheel, { passive: false });
    this.off.push(() => {
      canvas.removeEventListener("pointerdown", down);
      canvas.removeEventListener("pointermove", move);
      canvas.removeEventListener("pointerup", up);
      canvas.removeEventListener("pointercancel", up);
      canvas.removeEventListener("wheel", wheel);
    });
    const loop = () => {
      this.draw();
      this.frame = requestAnimationFrame(loop);
    };
    this.frame = requestAnimationFrame(loop);
  }

  setSkin(img: ImageData | null, slim: boolean) {
    this.skin = img ? shaded(img) : null;
    this.model = parts(slim, (img?.height ?? 64) >= 64);
  }

  setCape(img: ImageData | null) {
    this.cape = img ? shaded(img) : null;
    this.capeScale = img ? img.width / 64 : 1;
  }

  setPose(p: Pose) {
    this.pose = p;
  }

  dispose() {
    cancelAnimationFrame(this.frame);
    this.off.forEach((f) => f());
  }

  /// Углы частей в момент t (секунды) — движения как в игре.
  private angles(t: number): { parts: Record<string, Angles>; lift: number; tilt: number; cape: number } {
    const s = (k: number) => Math.sin(t * k);
    switch (this.pose) {
      case "idle":
        return {
          parts: {
            rArm: { x: 0, z: -0.04 - s(1.3) * 0.03 },
            lArm: { x: 0, z: 0.04 + s(1.3) * 0.03 },
            head: { x: s(0.6) * 0.04, y: s(0.4) * 0.15 },
          },
          lift: 0,
          tilt: 0,
          cape: 0.1 + s(1.3) * 0.02,
        };
      case "walk": {
        const w = s(6.5);
        return {
          parts: {
            rLeg: { x: w * 0.6 },
            lLeg: { x: -w * 0.6 },
            rArm: { x: -w * 0.55, z: -0.03 },
            lArm: { x: w * 0.55, z: 0.03 },
            head: { x: 0, y: s(1.1) * 0.1 },
          },
          lift: Math.abs(Math.cos(t * 6.5)) * 0.4,
          tilt: 0,
          cape: 0.25 + Math.abs(w) * 0.12,
        };
      }
      case "run": {
        const w = s(11);
        return {
          parts: {
            rLeg: { x: w * 1.05 },
            lLeg: { x: -w * 1.05 },
            rArm: { x: -w * 1.1, z: -0.08 },
            lArm: { x: w * 1.1, z: 0.08 },
            head: { x: -0.1 },
          },
          lift: Math.abs(Math.cos(t * 11)) * 1.2,
          tilt: 0.12,
          cape: 0.7 + Math.abs(w) * 0.2,
        };
      }
      case "fly":
        return {
          parts: {
            rArm: { x: 0, z: -0.25 - s(2) * 0.06 },
            lArm: { x: 0, z: 0.25 + s(2) * 0.06 },
            rLeg: { x: 0.05 + s(2.4) * 0.05 },
            lLeg: { x: 0.05 - s(2.4) * 0.05 },
            head: { x: -1.1 },
          },
          lift: 10 + s(1.6) * 0.8,
          tilt: 1.25,
          cape: 1.25 + s(2) * 0.05,
        };
    }
  }

  private draw() {
    const { canvas, ctx } = this;
    const dpr = window.devicePixelRatio || 1;
    const cw = canvas.clientWidth;
    const ch = canvas.clientHeight;
    if (canvas.width !== Math.round(cw * dpr) || canvas.height !== Math.round(ch * dpr)) {
      canvas.width = Math.round(cw * dpr);
      canvas.height = Math.round(ch * dpr);
    }
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (!this.skin) return;

    const t = (performance.now() - this.start) / 1000;
    const pose = this.angles(t);
    const scale = (Math.min(cw / 30, ch / 44) * this.zoom * dpr);
    const cx = canvas.width / 2;
    const cy = canvas.height / 2;
    // Перспектива слабая: камера далеко.
    const dist = 120;
    const project = (p: V3): [number, number, number] => {
      let q = rotY(p, this.yaw);
      q = rotX(q, this.pitch);
      const k = dist / (dist - q[2]);
      return [cx + q[0] * scale * k, cy - (q[1] - 16) * scale * k, q[2]];
    };
    // Поза всей фигуры: подъём (бег, полёт) и наклон вперёд вокруг пояса.
    const body = (p: V3): V3 => {
      const hip: V3 = [0, 12, 0];
      const q = add(rotX(sub(p, hip), pose.tilt), hip);
      return [q[0], q[1] + pose.lift - (this.pose === "fly" ? 10 : 0), q[2]];
    };

    const list: { pts: [number, number, number][]; face: Face; shade: number; depth: number }[] = [];
    const take = (name: string, part: Part, a: Angles | undefined) => {
      const tf = (p: V3): V3 => {
        let q = sub(p, part.pivot);
        if (a) {
          if (a.z) q = rotZ(q, a.z);
          if (a.y) q = rotY(q, a.y);
          q = rotX(q, a.x);
        }
        return body(add(q, part.pivot));
      };
      const tn = (n: V3): V3 => {
        let q = n;
        if (a) {
          if (a.z) q = rotZ(q, a.z);
          if (a.y) q = rotY(q, a.y);
          q = rotX(q, a.x);
        }
        q = rotX(q, pose.tilt);
        return q;
      };
      for (const face of part.faces) {
        if (face.tex === "cape" && !this.cape) continue;
        const n = tn(face.normal);
        // Камера: нормаль после поворота камеры смотрит на нас — грань видна.
        const nc = rotX(rotY(n, this.yaw), this.pitch);
        if (nc[2] <= 0.0001) continue;
        const pts = face.p.map((p) => project(tf(p))) as [number, number, number][];
        const light = Math.max(0, n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]);
        const shade = Math.min(SHADES.length - 1, Math.floor(light * SHADES.length));
        // Глубина — по центру грани (среднее диагонали).
        const c = tf(face.p[1]);
        const d = tf(face.p[2]);
        const mid: V3 = [(c[0] + d[0]) / 2, (c[1] + d[1]) / 2, (c[2] + d[2]) / 2];
        list.push({ pts, face, shade, depth: rotX(rotY(mid, this.yaw), this.pitch)[2] });
      }
      void name;
    };
    for (const [name, part] of Object.entries(this.model)) take(name, part, pose.parts[name]);
    take("cape", this.capeModel, { x: pose.cape });

    list.sort((a, b) => a.depth - b.depth);
    ctx.imageSmoothingEnabled = false;
    for (const { pts, face, shade } of list) {
      const tex = face.tex === "cape" ? this.cape![shade] : this.skin[shade];
      const k = face.tex === "cape" ? this.capeScale : 1;
      const [u, v, w, h] = face.uv;
      const [p0, p1, p2] = pts;
      // Текстура w×h → параллелограмм p0-p1-p2; чуть с запасом, чтобы
      // между гранями не светились щели.
      ctx.setTransform((p1[0] - p0[0]) / w, (p1[1] - p0[1]) / w, (p2[0] - p0[0]) / h, (p2[1] - p0[1]) / h, p0[0], p0[1]);
      ctx.drawImage(tex, u * k, v * k, w * k, h * k, -0.02, -0.02, w + 0.04, h + 0.04);
    }
  }
}
