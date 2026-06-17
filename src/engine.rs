use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Clone, Copy, Default)]
pub struct Note {
    pub slice: u8,
    pub semis: i8,
}

pub struct SharedState {
    pub current_step: AtomicUsize,
    pub vu: AtomicU32,
    pub pattern: Mutex<[Option<Note>; 16]>,
}

impl SharedState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            current_step: AtomicUsize::new(0),
            vu: AtomicU32::new(0),
            pattern: Mutex::new([None; 16]),
        })
    }

    pub fn vu_level(&self) -> f32 {
        f32::from_bits(self.vu.load(Ordering::Relaxed))
    }
}

pub enum Cmd {
    SetBpm(f32),
    SetPattern([Option<Note>; 16]),
    SetStep(usize, Option<Note>),
    Trigger(Note),
    Play(bool),
    Record(bool),
    Clear,
    LoadSample(Arc<Vec<f32>>, Vec<(usize, usize)>),
}

#[derive(Clone, Copy)]
struct Voice {
    pos: f32,
    start: f32,
    end: f32,
    speed: f32,
    active: bool,
}

pub struct Engine {
    sample: Arc<Vec<f32>>,
    slices: Vec<(usize, usize)>,
    pattern: [Option<Note>; 16],
    voices: [Voice; 16],
    sr: f32,
    src_ratio: f32,
    bpm: f32,
    samples_per_step: f32,
    step_acc: f32,
    step: usize,
    playing: bool,
    recording: bool,
    atk: f32,
    rel: f32,
    vu_peak: f32,
    shared: Arc<SharedState>,
}

impl Engine {
    pub fn new(
        sample: Arc<Vec<f32>>,
        slices: Vec<(usize, usize)>,
        sr: f32,
        src_ratio: f32,
        bpm: f32,
        shared: Arc<SharedState>,
    ) -> Self {
        let src_sr = sr * src_ratio;
        let mut e = Engine {
            sample,
            slices,
            pattern: [None; 16],
            voices: [Voice { pos: 0.0, start: 0.0, end: 0.0, speed: 1.0, active: false }; 16],
            sr,
            src_ratio,
            bpm,
            samples_per_step: 0.0,
            step_acc: 0.0,
            step: 0,
            playing: true,
            recording: false,
            atk: src_sr * 0.003,
            rel: src_sr * 0.005,
            vu_peak: 0.0,
            shared,
        };
        e.recalc();
        e
    }

    fn recalc(&mut self) {
        self.samples_per_step = self.sr * 60.0 / self.bpm / 4.0;
    }

    fn push_pattern(&self) {
        if let Ok(mut p) = self.shared.pattern.try_lock() {
            *p = self.pattern;
        }
    }

    pub fn apply(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::SetBpm(b) => {
                self.bpm = b.max(20.0);
                self.recalc();
            }
            Cmd::SetPattern(p) => {
                self.pattern = p;
                self.push_pattern();
            }
            Cmd::SetStep(i, v) => {
                if i < 16 {
                    self.pattern[i] = v;
                    self.push_pattern();
                }
            }
            Cmd::Trigger(n) => {
                self.trigger(n);
                if self.recording && self.playing {
                    let into = self.nearest_step();
                    self.pattern[into] = Some(n);
                    self.push_pattern();
                }
            }
            Cmd::Play(p) => self.playing = p,
            Cmd::Record(r) => self.recording = r,
            Cmd::Clear => {
                self.pattern = [None; 16];
                self.push_pattern();
            }
            Cmd::LoadSample(sample, slices) => {
                self.sample = sample;
                self.slices = slices;
                for v in self.voices.iter_mut() {
                    v.active = false;
                }
            }
        }
    }

    fn nearest_step(&self) -> usize {
        let elapsed = self.samples_per_step - self.step_acc;
        if elapsed > self.samples_per_step * 0.5 {
            self.step
        } else {
            (self.step + 15) % 16
        }
    }

    fn trigger(&mut self, note: Note) {
        let Some(&(s, e)) = self.slices.get(note.slice as usize) else {
            return;
        };
        let i = self.voices.iter().position(|v| !v.active).unwrap_or(0);
        let v = &mut self.voices[i];
        v.pos = s as f32;
        v.start = s as f32;
        v.end = e as f32;
        v.speed = self.src_ratio * 2.0f32.powf(note.semis as f32 / 12.0);
        v.active = true;
    }

    fn env(v: &Voice, atk: f32, rel: f32) -> f32 {
        let mut g = 1.0;
        let from_start = v.pos - v.start;
        if from_start < atk {
            g *= (from_start / atk).max(0.0);
        }
        let to_end = v.end - v.pos;
        if to_end < rel {
            g *= (to_end / rel).max(0.0);
        }
        g
    }

    fn read(sample: &[f32], pos: f32) -> f32 {
        let i = pos as usize;
        let frac = pos - i as f32;
        let a = sample[i];
        let b = if i + 1 < sample.len() { sample[i + 1] } else { a };
        a + (b - a) * frac
    }

    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        for frame in out.chunks_mut(channels) {
            if self.playing && self.step_acc <= 0.0 {
                self.shared.current_step.store(self.step, Ordering::Relaxed);
                if let Some(n) = self.pattern[self.step] {
                    self.trigger(n);
                }
                self.step = (self.step + 1) % 16;
                self.step_acc += self.samples_per_step;
            }
            if self.playing {
                self.step_acc -= 1.0;
            }

            let mut mix = 0.0f32;
            for v in self.voices.iter_mut() {
                if v.active {
                    mix += Self::read(&self.sample, v.pos) * Self::env(v, self.atk, self.rel);
                    v.pos += v.speed;
                    if v.pos >= v.end {
                        v.active = false;
                    }
                }
            }
            mix = (mix * 0.7).clamp(-1.0, 1.0);

            self.vu_peak = (self.vu_peak * 0.9997).max(mix.abs());
            self.shared.vu.store(self.vu_peak.to_bits(), Ordering::Relaxed);

            for s in frame.iter_mut() {
                *s = mix;
            }
        }
    }
}
