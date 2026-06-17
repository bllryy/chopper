use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use eframe::egui;
use egui::{Align2, Color32, FontId, Margin, Pos2, Rect, Rounding, Sense, Shadow, Stroke, Vec2};

use crate::engine::{Cmd, Note, SharedState};
use crate::util::{load_wav, slice_transient};

// ── DOS light-mode palette (Windows 3.1 / MS-DOS silver) ─────────────────────

const BG: Color32 = Color32::from_rgb(160, 160, 160);      // screen background, slightly darker silver
const PANEL: Color32 = Color32::from_rgb(192, 192, 192);   // classic Windows 3.1 silver
const PANEL2: Color32 = Color32::from_rgb(210, 210, 210);  // lighter panel / button face
const WAVE_BG: Color32 = Color32::from_rgb(230, 230, 230); // near-white waveform area

const HDR_BG: Color32 = Color32::from_rgb(0, 0, 128);      // navy DOS title bar
const HDR_BG_LT: Color32 = Color32::from_rgb(85, 85, 255); // bright blue accent on headers
const BORDER_HI: Color32 = Color32::WHITE;                  // raised button highlight (top/left)
const BORDER_LO: Color32 = Color32::from_rgb(80, 80, 80);  // raised button shadow (bottom/right)
const BORDER_MID: Color32 = Color32::from_rgb(128, 128, 128);

const TEXT: Color32 = Color32::from_rgb(0, 0, 0);          // black body text
const TEXT_HDR: Color32 = Color32::WHITE;                   // white text on blue headers
const TEXT_DIM: Color32 = Color32::from_rgb(110, 110, 110);
const TEXT_ACTIVE: Color32 = Color32::from_rgb(0, 100, 0); // dark green for active items
const TEXT_CUR: Color32 = Color32::from_rgb(120, 80, 0);   // dark gold for current step
const TEXT_CYAN: Color32 = Color32::from_rgb(0, 0, 160);   // dark blue (replaces bright cyan)
const TEXT_RED: Color32 = Color32::from_rgb(160, 0, 0);    // dark red

const STEP_ROW_EMPTY: Color32 = Color32::from_rgb(200, 200, 200);
const STEP_ROW_FULL: Color32 = Color32::from_rgb(185, 215, 185);  // light green for filled steps
const STEP_ROW_CUR: Color32 = Color32::from_rgb(215, 210, 155);   // light gold for current step
const STEP_ROW_FLASH: Color32 = Color32::from_rgb(160, 220, 140); // brighter flash

const VU_GREEN: Color32 = Color32::from_rgb(0, 150, 0);
const VU_YELLOW: Color32 = Color32::from_rgb(160, 140, 0);
const VU_RED: Color32 = Color32::from_rgb(180, 0, 0);
const VU_OFF: Color32 = Color32::from_rgb(170, 170, 170);

const WAVE_PTS: usize = 2048;

// ── Text / font helpers ───────────────────────────────────────────────────────

const FONT_SM: f32 = 9.5;
const FONT_MD: f32 = 10.5;

fn fnt() -> FontId { FontId::monospace(FONT_SM) }
fn fnt_md() -> FontId { FontId::monospace(FONT_MD) }

fn txt(p: &egui::Painter, s: &str, pos: Pos2, col: Color32) {
    p.text(pos, Align2::LEFT_TOP, s, fnt(), col);
}

fn txt_center(p: &egui::Painter, s: &str, rect: Rect, col: Color32) {
    p.text(rect.center(), Align2::CENTER_CENTER, s, fnt(), col);
}

fn txt_md_center(p: &egui::Painter, s: &str, rect: Rect, col: Color32) {
    p.text(rect.center(), Align2::CENTER_CENTER, s, fnt_md(), col);
}

// ── DOS UI primitives ─────────────────────────────────────────────────────────

/// Filled rect with hard pixel border.
fn dos_rect(p: &egui::Painter, rect: Rect, fill: Color32, border: Color32) {
    p.rect_filled(rect, Rounding::ZERO, fill);
    p.rect_stroke(rect, Rounding::ZERO, Stroke::new(1.0_f32, border));
}

/// Classic DOS "raised" border — white top/left, gray bottom/right.
fn raised(p: &egui::Painter, rect: Rect) {
    // outer highlight
    p.line_segment([rect.left_top(), Pos2::new(rect.right() - 1.0, rect.top())], Stroke::new(1.0_f32, BORDER_HI));
    p.line_segment([rect.left_top(), Pos2::new(rect.left(), rect.bottom() - 1.0)], Stroke::new(1.0_f32, BORDER_HI));
    // outer shadow
    p.line_segment([Pos2::new(rect.left(), rect.bottom()), rect.right_bottom()], Stroke::new(1.0_f32, BORDER_LO));
    p.line_segment([Pos2::new(rect.right(), rect.top()), rect.right_bottom()], Stroke::new(1.0_f32, BORDER_LO));
}

/// Pressed (inset) border — dark top/left, white bottom/right.
fn pressed(p: &egui::Painter, rect: Rect) {
    p.line_segment([rect.left_top(), Pos2::new(rect.right() - 1.0, rect.top())], Stroke::new(1.0_f32, BORDER_LO));
    p.line_segment([rect.left_top(), Pos2::new(rect.left(), rect.bottom() - 1.0)], Stroke::new(1.0_f32, BORDER_LO));
    p.line_segment([Pos2::new(rect.left(), rect.bottom()), rect.right_bottom()], Stroke::new(1.0_f32, BORDER_HI));
    p.line_segment([Pos2::new(rect.right(), rect.top()), rect.right_bottom()], Stroke::new(1.0_f32, BORDER_HI));
}

/// Blue header bar — navy fill, white text, bright-blue top/left border.
fn header_bar(p: &egui::Painter, rect: Rect, label: &str) {
    p.rect_filled(rect, Rounding::ZERO, HDR_BG);
    p.line_segment([rect.left_top(), Pos2::new(rect.right(), rect.top())], Stroke::new(1.0_f32, HDR_BG_LT));
    p.line_segment([rect.left_top(), Pos2::new(rect.left(), rect.bottom())], Stroke::new(1.0_f32, HDR_BG_LT));
    txt_md_center(p, label, rect, TEXT_HDR);
}

/// Button: raised silver, black text. Returns true if clicked.
fn dos_btn(ui: &mut egui::Ui, label: &str, w: f32, h: f32) -> bool {
    dos_btn_col(ui, label, w, h, PANEL2, TEXT)
}

fn dos_btn_col(ui: &mut egui::Ui, label: &str, w: f32, h: f32, fill: Color32, tcol: Color32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let is_down = resp.is_pointer_button_down_on();
    let bg = if is_down { fill.gamma_multiply(0.9) } else if resp.hovered() { fill.gamma_multiply(1.08) } else { fill };
    ui.painter().rect_filled(rect, Rounding::ZERO, bg);
    if is_down { pressed(ui.painter(), rect); } else { raised(ui.painter(), rect); }
    txt_center(ui.painter(), label, rect, tcol);
    resp.clicked()
}

// ── App struct ────────────────────────────────────────────────────────────────

pub struct ChopperApp {
    pub tx: rtrb::Producer<Cmd>,
    pub shared: Arc<SharedState>,

    peaks: Vec<(f32, f32)>,
    slices: Vec<(usize, usize)>,
    sample_len: usize,
    sample_name: String,

    selected_slice: u8,
    octave: i8,
    bpm: f32,
    playing: bool,
    recording: bool,

    pad_flash: [f32; 16],
    step_flash: [f32; 16],
    last_step: usize,
    font_loaded: bool,
}

impl ChopperApp {
    pub fn new(
        tx: rtrb::Producer<Cmd>,
        shared: Arc<SharedState>,
        sample: &[f32],
        slices: Vec<(usize, usize)>,
        sample_name: String,
        bpm: f32,
    ) -> Self {
        Self {
            tx, shared,
            peaks: compute_peaks(sample),
            slices,
            sample_len: sample.len(),
            sample_name,
            selected_slice: 0,
            octave: 0,
            bpm,
            playing: true,
            recording: false,
            pad_flash: [0.0; 16],
            step_flash: [0.0; 16],
            last_step: 99,
            font_loaded: false,
        }
    }

    fn send(&mut self, cmd: Cmd) { self.tx.push(cmd).ok(); }

    fn trigger_pad(&mut self, idx: u8) {
        self.selected_slice = idx;
        self.send(Cmd::Trigger(Note { slice: idx, semis: self.octave * 12 }));
        if (idx as usize) < 16 { self.pad_flash[idx as usize] = 1.0; }
    }

    fn load_file(&mut self, path: &Path) {
        let s = path.to_string_lossy();
        if let Some((sample, wsr)) = load_wav(&s) {
            let slices = slice_transient(&sample, wsr as f32);
            self.peaks = compute_peaks(&sample);
            self.sample_len = sample.len();
            self.sample_name = path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| s.into_owned());
            self.slices = slices.clone();
            self.send(Cmd::LoadSample(sample, slices));
        }
    }
}

fn compute_peaks(sample: &[f32]) -> Vec<(f32, f32)> {
    if sample.is_empty() { return vec![(0.0, 0.0); WAVE_PTS]; }
    (0..WAVE_PTS).map(|i| {
        let a = i * sample.len() / WAVE_PTS;
        let b = ((i + 1) * sample.len() / WAVE_PTS).min(sample.len());
        sample[a..b].iter().fold((0.0f32, 0.0f32), |(mn, mx), &s| (mn.min(s), mx.max(s)))
    }).collect()
}

fn note_name(semis: i8) -> String {
    let names = ["C-","C#","D-","D#","E-","F-","F#","G-","G#","A-","A#","B-"];
    format!("{}{:+}", names[semis.rem_euclid(12) as usize], semis / 12)
}

// ── Style ─────────────────────────────────────────────────────────────────────

fn setup_style(ctx: &egui::Context) {
    let mut vis = egui::Visuals::light();
    vis.window_rounding = Rounding::ZERO;
    vis.menu_rounding = Rounding::ZERO;
    vis.window_shadow = Shadow::NONE;
    vis.popup_shadow = Shadow::NONE;
    vis.window_fill = PANEL;
    vis.panel_fill = PANEL;
    vis.override_text_color = Some(TEXT);
    vis.selection.bg_fill = Color32::from_rgb(0, 0, 200);

    let mk = |bg: Color32, bdr: Color32| egui::style::WidgetVisuals {
        bg_fill: bg, weak_bg_fill: bg,
        bg_stroke: Stroke::new(1.0_f32, bdr),
        rounding: Rounding::ZERO,
        fg_stroke: Stroke::new(1.0_f32, TEXT),
        expansion: 0.0,
    };
    vis.widgets.noninteractive = mk(PANEL, BORDER_MID);
    vis.widgets.inactive     = mk(PANEL2, BORDER_MID);
    vis.widgets.hovered      = mk(PANEL2, HDR_BG);
    vis.widgets.active       = mk(PANEL, BORDER_LO);
    vis.widgets.open         = mk(PANEL, BORDER_MID);
    ctx.set_visuals(vis);

    let mut sty = (*ctx.style()).clone();
    sty.spacing.item_spacing = Vec2::new(2.0, 1.0);
    sty.spacing.button_padding = Vec2::new(4.0, 2.0);
    sty.spacing.window_margin = Margin::same(0.0);
    ctx.set_style(sty);
}

// ── eframe::App ───────────────────────────────────────────────────────────────

impl eframe::App for ChopperApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // One-time font load
        if !self.font_loaded {
            ctx.tessellation_options_mut(|o| o.feathering = false);
            let path = "/home/lily/.local/share/fonts/IosevkaNerdFontMono-Regular.ttf";
            if let Ok(bytes) = std::fs::read(path) {
                let mut fonts = egui::FontDefinitions::default();
                fonts.font_data.insert("iosevka".into(), egui::FontData::from_owned(bytes));
                for fam in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
                    fonts.families.entry(fam).or_default().insert(0, "iosevka".into());
                }
                ctx.set_fonts(fonts);
            }
            self.font_loaded = true;
        }

        setup_style(ctx);
        ctx.request_repaint();

        let dt = ctx.input(|i| i.unstable_dt).min(0.1);
        for f in self.pad_flash.iter_mut() { *f = (*f - dt * 7.0).max(0.0); }
        for f in self.step_flash.iter_mut() { *f = (*f - dt * 10.0).max(0.0); }

        // File drop
        let dropped: Vec<_> = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(f) = dropped.first() {
            if let Some(p) = &f.path { self.load_file(p); }
        }
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());

        // Step advance flash
        let cur = self.shared.current_step.load(Ordering::Relaxed);
        if cur != self.last_step { self.step_flash[cur] = 1.0; self.last_step = cur; }

        let pattern = self.shared.pattern.lock().map(|p| *p).unwrap_or([None; 16]);
        let vu = self.shared.vu_level();

        self.handle_keys(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG).inner_margin(Margin::same(3.0)))
            .show(ctx, |ui| {
                ui.set_clip_rect(ui.max_rect());

                self.draw_title_bar(ui, vu);
                ui.add_space(3.0);

                let avail = ui.available_size();
                let lw = (avail.x * 0.44).max(260.0);
                let rw = avail.x - lw - 4.0;
                let mh = (avail.y - 120.0).max(140.0);

                ui.horizontal_top(|ui| {
                    ui.allocate_ui(Vec2::new(lw, mh), |ui| self.draw_waveform(ui, cur, &pattern));
                    ui.add_space(4.0);
                    ui.allocate_ui(Vec2::new(rw, mh), |ui| self.draw_pattern(ui, &pattern, cur));
                });

                ui.add_space(3.0);
                self.draw_pads(ui);

                if hovering {
                    let r = ctx.screen_rect();
                    let lay = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop"));
                    let p = ctx.layer_painter(lay);
                    p.rect_filled(r, Rounding::ZERO, Color32::from_rgba_unmultiplied(192, 192, 192, 210));
                    p.rect_stroke(r.shrink(4.0), Rounding::ZERO, Stroke::new(2.0_f32, HDR_BG));
                    p.text(r.center(), Align2::CENTER_CENTER, "DROP WAV FILE", fnt_md(), HDR_BG);
                }
            });
    }
}

// ── Title bar ─────────────────────────────────────────────────────────────────

impl ChopperApp {
    fn draw_title_bar(&mut self, ui: &mut egui::Ui, vu: f32) {
        let aw = ui.available_width();

        // Blue title band
        let (title_rect, _) = ui.allocate_exact_size(Vec2::new(aw, 14.0), Sense::hover());
        let p = ui.painter();
        header_bar(p, title_rect, "");
        txt(p, "  CHOPPER  v0.1", title_rect.left_top() + Vec2::new(2.0, 2.0), TEXT_HDR);
        let slice_info = format!("[{} slices]  {}", self.slices.len(), &self.sample_name[..self.sample_name.len().min(28)]);
        txt(p, &slice_info, title_rect.left_top() + Vec2::new(120.0, 2.0), Color32::from_rgb(170, 200, 255));

        ui.add_space(2.0);

        // Controls row
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            if dos_btn(ui, "LOAD", 42.0, 14.0) {
                if let Some(p) = rfd::FileDialog::new().add_filter("WAV", &["wav","WAV"]).pick_file() {
                    self.load_file(&p);
                }
            }
            ui.add_space(6.0);

            // BPM display + +/- buttons
            txt(ui.painter(), " BPM:", Pos2::new(ui.next_widget_position().x, ui.next_widget_position().y + 2.0), TEXT_CYAN);
            let _ = ui.allocate_exact_size(Vec2::new(32.0, 14.0), Sense::hover());

            let bpm_rect = ui.allocate_exact_size(Vec2::new(36.0, 14.0), Sense::hover()).0;
            dos_rect(ui.painter(), bpm_rect, PANEL2, BORDER_LO);
            txt_center(ui.painter(), &format!("{:.0}", self.bpm), bpm_rect, TEXT);

            if dos_btn(ui, "-", 14.0, 14.0) {
                self.bpm = (self.bpm - 1.0).max(20.0);
                self.send(Cmd::SetBpm(self.bpm));
            }
            if dos_btn(ui, "+", 14.0, 14.0) {
                self.bpm = (self.bpm + 1.0).min(300.0);
                self.send(Cmd::SetBpm(self.bpm));
            }
            ui.add_space(8.0);

            // Play/Stop
            let (plabel, pfill, ptcol) = if self.playing {
                ("STOP", Color32::from_rgb(180, 100, 0), TEXT_HDR)
            } else {
                ("PLAY", Color32::from_rgb(0, 100, 0), TEXT_HDR)
            };
            if dos_btn_col(ui, plabel, 44.0, 14.0, pfill, ptcol) {
                self.playing = !self.playing;
                self.send(Cmd::Play(self.playing));
            }

            let (rfill, rtcol) = if self.recording {
                (Color32::from_rgb(160, 0, 0), TEXT_HDR)
            } else {
                (PANEL2, TEXT_RED)
            };
            if dos_btn_col(ui, "REC", 38.0, 14.0, rfill, rtcol) {
                self.recording = !self.recording;
                self.send(Cmd::Record(self.recording));
            }

            if dos_btn(ui, "CLR", 38.0, 14.0) {
                self.send(Cmd::Clear);
            }

            ui.add_space(8.0);

            // Octave
            txt(ui.painter(), &format!("OCT{:+}", self.octave),
                Pos2::new(ui.next_widget_position().x, ui.next_widget_position().y + 2.0), TEXT_CYAN);
            let _ = ui.allocate_exact_size(Vec2::new(46.0, 14.0), Sense::hover());
            if dos_btn(ui, "DN", 26.0, 14.0) { self.octave = (self.octave - 1).max(-3); }
            if dos_btn(ui, "UP", 26.0, 14.0) { self.octave = (self.octave + 1).min(3); }

            ui.add_space(8.0);

            // VU meter — segmented
            txt(ui.painter(), "VU:", Pos2::new(ui.next_widget_position().x, ui.next_widget_position().y + 2.0), TEXT_DIM);
            let _ = ui.allocate_exact_size(Vec2::new(24.0, 14.0), Sense::hover());
            let vu_w = 120.0;
            let (vu_r, _) = ui.allocate_exact_size(Vec2::new(vu_w, 14.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(vu_r, Rounding::ZERO, PANEL);
            raised(p, vu_r);
            let inner = vu_r.shrink(2.0);
            let seg = 6.0; let gap = 1.0;
            let filled = (vu * inner.width()).min(inner.width());
            let mut x = inner.left();
            while x + seg - gap <= inner.right() {
                let frac = (x - inner.left()) / inner.width();
                let on = x + seg - gap <= inner.left() + filled;
                let col = if on {
                    if frac > 0.85 { VU_RED } else if frac > 0.6 { VU_YELLOW } else { VU_GREEN }
                } else { VU_OFF };
                p.rect_filled(
                    Rect::from_min_size(Pos2::new(x, inner.top()), Vec2::new(seg - gap, inner.height())),
                    Rounding::ZERO, col,
                );
                x += seg;
            }
        });
    }

// ── Waveform panel ────────────────────────────────────────────────────────────

    fn draw_waveform(&mut self, ui: &mut egui::Ui, cur_step: usize, pattern: &[Option<Note>; 16]) {
        let avail = ui.available_size();
        let (outer, _) = ui.allocate_exact_size(avail, Sense::hover());
        let p = ui.painter();

        let hh = 13.0;
        header_bar(p, Rect::from_min_size(outer.min, Vec2::new(outer.width(), hh)), "SAMPLE WAVEFORM");

        let wave_rect = Rect::from_min_max(
            Pos2::new(outer.left(), outer.top() + hh + 1.0),
            Pos2::new(outer.right(), outer.bottom() - 13.0),
        );
        p.rect_filled(wave_rect, Rounding::ZERO, WAVE_BG);
        raised(p, wave_rect);

        let w = wave_rect.width() as usize;
        let cy = wave_rect.center().y;
        let hf = wave_rect.height() * 0.44;

        if w > 0 && !self.peaks.is_empty() {
            // center line
            p.line_segment(
                [Pos2::new(wave_rect.left(), cy), Pos2::new(wave_rect.right(), cy)],
                Stroke::new(1.0_f32, Color32::from_rgb(180, 180, 180)),
            );
            // waveform
            for x in 0..w {
                let pi = (x * WAVE_PTS / w).min(WAVE_PTS - 1);
                let (mn, mx) = self.peaks[pi];
                let y0 = cy - mx * hf;
                let y1 = (cy - mn * hf).max(y0 + 1.0);
                p.line_segment(
                    [Pos2::new(wave_rect.left() + x as f32, y0), Pos2::new(wave_rect.left() + x as f32, y1)],
                    Stroke::new(1.0_f32, Color32::from_rgb(0, 120, 0)),
                );
            }
            // current step highlight
            if let Some(note) = pattern[cur_step] {
                if let Some(&(s, e)) = self.slices.get(note.slice as usize) {
                    let x0 = wave_rect.left() + (s as f32 / self.sample_len as f32) * wave_rect.width();
                    let x1 = wave_rect.left() + (e as f32 / self.sample_len as f32) * wave_rect.width();
                    let fl = self.step_flash[cur_step];
                    if fl > 0.01 {
                        p.rect_filled(
                            Rect::from_x_y_ranges(x0..=x1, wave_rect.top()..=wave_rect.bottom()),
                            Rounding::ZERO,
                            Color32::from_rgba_unmultiplied(0, 150, 0, (fl * 70.0) as u8),
                        );
                    }
                }
            }
            // slice markers
            for (si, &(start, _)) in self.slices.iter().enumerate() {
                if si >= 16 { break; }
                let x = wave_rect.left() + (start as f32 / self.sample_len as f32) * wave_rect.width();
                let is_sel = si == self.selected_slice as usize;
                let lw = if is_sel { 2.0_f32 } else { 1.0_f32 };
                let col = if is_sel { HDR_BG } else { Color32::from_rgb(100, 100, 200) };
                p.line_segment([Pos2::new(x, wave_rect.top()), Pos2::new(x, wave_rect.bottom())], Stroke::new(lw, col));
                p.text(Pos2::new(x + 1.0, wave_rect.top() + 1.0), Align2::LEFT_TOP,
                    &format!("{:02}", si), FontId::monospace(7.5),
                    if is_sel { HDR_BG } else { Color32::from_rgb(120, 120, 180) });
            }
        }

        // footer
        let foot = Rect::from_min_size(Pos2::new(outer.left(), outer.bottom() - 12.0), Vec2::new(outer.width(), 12.0));
        p.rect_filled(foot, Rounding::ZERO, PANEL);
        raised(p, foot);
        txt(p, &format!("  {}  SL:{:02}/{:02}", &self.sample_name[..self.sample_name.len().min(24)],
            self.selected_slice, self.slices.len().saturating_sub(1)),
            Pos2::new(foot.left() + 3.0, foot.top() + 1.5), TEXT_DIM);
    }

// ── Pattern editor ────────────────────────────────────────────────────────────

    fn draw_pattern(&mut self, ui: &mut egui::Ui, pattern: &[Option<Note>; 16], cur_step: usize) {
        let avail = ui.available_size();
        let (outer, _) = ui.allocate_exact_size(avail, Sense::hover());
        let p = ui.painter();

        let hh = 13.0;
        header_bar(p, Rect::from_min_size(outer.min, Vec2::new(outer.width(), hh)), "PATTERN EDITOR");

        // Column headers
        let chh = 11.0;
        let chr = Rect::from_min_size(Pos2::new(outer.left(), outer.top() + hh + 1.0), Vec2::new(outer.width(), chh));
        p.rect_filled(chr, Rounding::ZERO, PANEL);
        raised(p, chr);
        let hw = outer.width() / 2.0 - 2.0;
        for col in 0..2 {
            let ox = outer.left() + col as f32 * (hw + 4.0);
            txt(p, " # | SL | NOTE", Pos2::new(ox + 2.0, chr.top() + 1.5), TEXT_CYAN);
        }

        let rows_top = chr.bottom() + 1.0;
        let row_h = ((outer.bottom() - rows_top - 13.0) / 8.0).max(12.0);

        let mp = ui.input(|i| i.pointer.interact_pos());
        let lclick = ui.input(|i| i.pointer.primary_clicked());
        let rclick = ui.input(|i| i.pointer.secondary_clicked());
        let scroll = ui.input(|i| i.raw_scroll_delta.y);

        for col in 0..2usize {
            let ox = outer.left() + col as f32 * (hw + 4.0);
            for row in 0..8usize {
                let step = col * 8 + row;
                let ry = rows_top + row as f32 * row_h;
                let rr = Rect::from_min_size(Pos2::new(ox, ry), Vec2::new(hw, row_h - 1.0));

                let is_cur = step == cur_step && self.playing;
                let flash = self.step_flash[step];
                let note = pattern[step];

                let bg = if is_cur {
                    lerp_col(STEP_ROW_CUR, STEP_ROW_FLASH, flash)
                } else if note.is_some() {
                    STEP_ROW_FULL
                } else {
                    STEP_ROW_EMPTY
                };
                p.rect_filled(rr, Rounding::ZERO, bg);

                if is_cur {
                    p.rect_stroke(rr, Rounding::ZERO, Stroke::new(1.5_f32, HDR_BG));
                } else {
                    p.rect_stroke(rr, Rounding::ZERO, Stroke::new(1.0_f32, BORDER_MID));
                }

                let nc = if is_cur { TEXT_CUR } else if note.is_some() { TEXT_ACTIVE } else { TEXT_DIM };
                txt(p, &format!("{:02}", step + 1), Pos2::new(rr.left() + 2.0, rr.top() + 1.5), nc);

                // separator
                p.line_segment([Pos2::new(rr.left() + 19.0, rr.top()), Pos2::new(rr.left() + 19.0, rr.bottom())],
                    Stroke::new(1.0_f32, BORDER_MID));

                if let Some(n) = note {
                    let dc = if is_cur { TEXT_CUR } else { TEXT_ACTIVE };
                    let pc = if is_cur { TEXT_CUR } else { TEXT_CYAN };
                    txt(p, &format!("{:02}", n.slice), Pos2::new(rr.left() + 22.0, rr.top() + 1.5), dc);
                    p.line_segment([Pos2::new(rr.left() + 38.0, rr.top()), Pos2::new(rr.left() + 38.0, rr.bottom())],
                        Stroke::new(1.0_f32, BORDER_MID));
                    txt(p, &note_name(n.semis), Pos2::new(rr.left() + 41.0, rr.top() + 1.5), pc);
                } else {
                    txt(p, "-- | ---", Pos2::new(rr.left() + 22.0, rr.top() + 1.5), TEXT_DIM);
                }

                if let Some(m) = mp {
                    if rr.contains(m) {
                        p.rect_stroke(rr, Rounding::ZERO, Stroke::new(1.5_f32, HDR_BG_LT));
                        if lclick {
                            if note.is_some() { self.send(Cmd::SetStep(step, None)); }
                            else { self.send(Cmd::SetStep(step, Some(Note { slice: self.selected_slice, semis: self.octave * 12 }))); }
                        } else if rclick {
                            self.send(Cmd::SetStep(step, None));
                        } else if scroll.abs() > 0.5 {
                            if let Some(n) = note {
                                self.send(Cmd::SetStep(step, Some(Note {
                                    slice: n.slice,
                                    semis: (n.semis as i32 + scroll.signum() as i32).clamp(-24, 24) as i8,
                                })));
                            }
                        }
                    }
                }
            }
        }

        // foot
        let fr = Rect::from_min_size(Pos2::new(outer.left(), outer.bottom() - 12.0), Vec2::new(outer.width(), 12.0));
        p.rect_filled(fr, Rounding::ZERO, PANEL);
        raised(p, fr);
        txt(p, " LMB=TOGGLE  RMB=DEL  SCROLL=PITCH", Pos2::new(fr.left() + 3.0, fr.top() + 1.5), TEXT_DIM);
    }

// ── Pads ──────────────────────────────────────────────────────────────────────

    fn draw_pads(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_size();
        let h = avail.y.min(115.0);
        let (outer, _) = ui.allocate_exact_size(Vec2::new(avail.x, h), Sense::hover());
        let p = ui.painter();

        let hh = 13.0;
        header_bar(p, Rect::from_min_size(outer.min, Vec2::new(outer.width(), hh)), "SAMPLE PADS");

        let area = Rect::from_min_max(Pos2::new(outer.left(), outer.top() + hh + 1.0), outer.max);
        p.rect_filled(area, Rounding::ZERO, PANEL);
        raised(p, area);

        let gap = 4.0;
        let pw = (area.width() - gap * 9.0) / 8.0;
        let ph = (area.height() - gap * 3.0) / 2.0;

        let mp = ui.input(|i| i.pointer.interact_pos());
        let md = ui.input(|i| i.pointer.primary_down());
        let mc = ui.input(|i| i.pointer.primary_clicked());
        let keys = "12345678QWERTYUI";

        for row in 0..2usize {
            for col in 0..8usize {
                let idx = (row * 8 + col) as u8;
                let flash = self.pad_flash[idx as usize];
                let is_sel = idx == self.selected_slice;

                let x = area.left() + gap + col as f32 * (pw + gap);
                let y = area.top() + gap + row as f32 * (ph + gap);
                let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(pw, ph));

                let hov = mp.map(|m| rect.contains(m)).unwrap_or(false);
                let dn = hov && md;

                let bg = if dn || flash > 0.5 {
                    Color32::from_rgb(100, 140, 220)  // blue-pressed
                } else if is_sel {
                    Color32::from_rgb(180, 200, 240)  // selected: light blue
                } else if hov {
                    Color32::from_rgb(205, 215, 225)  // hover: slightly blue-gray
                } else {
                    PANEL2
                };
                p.rect_filled(rect, Rounding::ZERO, bg);
                if dn { pressed(p, rect); } else { raised(p, rect); }

                // Extra blue border for selected
                if is_sel {
                    p.rect_stroke(rect.shrink(2.0), Rounding::ZERO, Stroke::new(1.0_f32, HDR_BG));
                }

                let nc = if dn || flash > 0.3 { TEXT_HDR }
                    else if is_sel { HDR_BG }
                    else { TEXT };
                p.text(Pos2::new(rect.center().x, rect.top() + rect.height() * 0.28),
                    Align2::CENTER_CENTER, &format!("{:02}", idx + 1), fnt_md(), nc);
                if let Some(k) = keys.chars().nth(idx as usize) {
                    p.text(Pos2::new(rect.center().x, rect.top() + rect.height() * 0.72),
                        Align2::CENTER_CENTER, &k.to_string(), fnt(), TEXT_DIM);
                }

                if hov && mc { self.trigger_pad(idx); }
            }
        }
    }

// ── Keyboard handling ─────────────────────────────────────────────────────────

    fn handle_keys(&mut self, ctx: &egui::Context) {
        use egui::Key;
        let pad_keys = [
            Key::Num1, Key::Num2, Key::Num3, Key::Num4,
            Key::Num5, Key::Num6, Key::Num7, Key::Num8,
            Key::Q, Key::W, Key::E, Key::R,
            Key::T, Key::Y, Key::U, Key::I,
        ];
        let all: &[Key] = &[
            Key::Num1, Key::Num2, Key::Num3, Key::Num4,
            Key::Num5, Key::Num6, Key::Num7, Key::Num8,
            Key::Q, Key::W, Key::E, Key::R, Key::T, Key::Y, Key::U, Key::I,
            Key::Space, Key::ArrowUp, Key::ArrowDown, Key::Tab, Key::Delete, Key::Backspace,
        ];
        let pressed: Vec<Key> = ctx.input(|i| all.iter().filter(|&&k| i.key_pressed(k)).copied().collect());
        for key in pressed {
            if let Some(i) = pad_keys.iter().position(|&k| k == key) {
                self.trigger_pad(i as u8);
            } else {
                match key {
                    Key::Space => { self.playing = !self.playing; self.send(Cmd::Play(self.playing)); }
                    Key::ArrowUp => { self.octave = (self.octave + 1).min(3); }
                    Key::ArrowDown => { self.octave = (self.octave - 1).max(-3); }
                    Key::Tab => { self.recording = !self.recording; self.send(Cmd::Record(self.recording)); }
                    Key::Delete | Key::Backspace => { self.send(Cmd::Clear); }
                    _ => {}
                }
            }
        }
    }
}

fn lerp_col(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}
