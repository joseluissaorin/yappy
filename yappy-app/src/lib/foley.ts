// EL FOLEY: los sonidos de la casa, sintetizados en el momento (sin
// ficheros): plops de pegatina, ticks de corona, rasgados de papel. Todo
// corto (<200 ms) y bajito: acompaña al dedo, jamás compite con la voz.

let ctx: AudioContext | null = null;

function audio(): AudioContext | null {
  try {
    if (!ctx) {
      const AC = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!AC) return null;
      ctx = new AC();
    }
    if (ctx.state === "suspended") void ctx.resume();
    return ctx;
  } catch {
    return null;
  }
}

/// Un tono breve que se desliza de f0 a f1 con caída exponencial.
function tono(f0: number, f1: number, dur: number, tipo: OscillatorType, gan = 0.12) {
  const ac = audio();
  if (!ac) return;
  const osc = ac.createOscillator();
  const g = ac.createGain();
  const t = ac.currentTime;
  osc.type = tipo;
  osc.frequency.setValueAtTime(f0, t);
  osc.frequency.exponentialRampToValueAtTime(Math.max(30, f1), t + dur);
  g.gain.setValueAtTime(gan, t);
  g.gain.exponentialRampToValueAtTime(0.001, t + dur);
  osc.connect(g).connect(ac.destination);
  osc.start(t);
  osc.stop(t + dur + 0.02);
}

/// Ruido corto filtrado: papel que se rasga o roza.
function papel(dur: number, freq: number, gan = 0.1) {
  const ac = audio();
  if (!ac) return;
  const n = Math.floor(ac.sampleRate * dur);
  const buf = ac.createBuffer(1, n, ac.sampleRate);
  const d = buf.getChannelData(0);
  for (let i = 0; i < n; i++) d[i] = (Math.random() * 2 - 1) * (1 - i / n);
  const src = ac.createBufferSource();
  src.buffer = buf;
  const filtro = ac.createBiquadFilter();
  filtro.type = "bandpass";
  filtro.frequency.value = freq;
  filtro.Q.value = 0.8;
  const g = ac.createGain();
  g.gain.value = gan;
  src.connect(filtro).connect(g).connect(ac.destination);
  src.start();
}

/// La pegatina que se PEGA (aterrizaje imantado, soltar la marca).
export function plop() {
  tono(340, 85, 0.09, "sine", 0.17);
}

/// Abrir algo pequeño (el sello de añadir, una pestaña).
export function pop() {
  tono(170, 540, 0.07, "triangle", 0.12);
}

/// El detent de la corona: un clic mínimo.
export function tick() {
  tono(1300, 1100, 0.02, "square", 0.04);
}

/// El papel que se desliza: abrir o cerrar el cajón de la trastienda.
export function rasga() {
  papel(0.16, 2400, 0.09);
  tono(220, 130, 0.12, "sine", 0.05);
}

/// La gelatina del «yappy»: un boing blandito.
export function boing() {
  tono(250, 160, 0.16, "sine", 0.15);
  tono(500, 320, 0.11, "sine", 0.05);
}

/// EL VUELO: un silbido que cae con el picado y remonta, con aleteo de
/// soplos cortos por debajo. La duración acompaña al recorrido.
export function vuelo(durMs: number) {
  const ac = audio();
  if (!ac) return;
  const dur = Math.min(3, durMs / 1000);
  const t = ac.currentTime;
  const osc = ac.createOscillator();
  const g = ac.createGain();
  osc.type = "sine";
  osc.frequency.setValueAtTime(760, t);
  osc.frequency.exponentialRampToValueAtTime(330, t + dur * 0.5);
  osc.frequency.exponentialRampToValueAtTime(820, t + dur);
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(0.055, t + 0.08);
  g.gain.setValueAtTime(0.055, t + dur * 0.8);
  g.gain.exponentialRampToValueAtTime(0.001, t + dur);
  osc.connect(g).connect(ac.destination);
  osc.start(t);
  osc.stop(t + dur + 0.03);
  // El aleteo: soplos de aire espaciados que se van apagando.
  const soplos = Math.floor(dur / 0.22);
  for (let i = 0; i < soplos; i++) {
    setTimeout(() => papel(0.05, 900, 0.05), i * 220);
  }
}
