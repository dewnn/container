let audioContext: AudioContext | null = null;

function context() {
  if (typeof window === "undefined") return null;
  audioContext ??= new AudioContext();
  return audioContext;
}

export function armCompletionSound() {
  const audio = context();
  if (audio?.state === "suspended") void audio.resume();
}

export async function playCompletionSound() {
  try {
    const audio = context();
    if (!audio) return;
    if (audio.state === "suspended") await audio.resume();

    const start = audio.currentTime + 0.025;
    const master = audio.createGain();
    master.gain.setValueAtTime(0.0001, start);
    master.gain.exponentialRampToValueAtTime(0.12, start + 0.018);
    master.gain.exponentialRampToValueAtTime(0.0001, start + 0.58);
    master.connect(audio.destination);

    for (const [frequency, delay, duration] of [
      [659.25, 0, 0.34],
      [987.77, 0.14, 0.43],
    ] as const) {
      const tone = audio.createOscillator();
      const envelope = audio.createGain();
      const noteStart = start + delay;
      tone.type = "sine";
      tone.frequency.setValueAtTime(frequency, noteStart);
      envelope.gain.setValueAtTime(0.0001, noteStart);
      envelope.gain.exponentialRampToValueAtTime(1, noteStart + 0.012);
      envelope.gain.exponentialRampToValueAtTime(0.0001, noteStart + duration);
      tone.connect(envelope);
      envelope.connect(master);
      tone.start(noteStart);
      tone.stop(noteStart + duration + 0.02);
    }
  } catch {
    // A completion sound is optional and must never affect a finished export.
  }
}
