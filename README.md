# CHOPPER

A 16-step drum machine and sample slicer with a DOS-tracker style GUI.

Loads a WAV file, detects transients, slices the audio, and lets you arrange those slices into a 16-step pattern — triggering pads, adjusting pitch per step, and recording in real time.

---

## Building

```
cargo build --release
```

Requires ALSA dev libraries on Linux (`alsa-lib-devel` on Fedora / `libasound2-dev` on Debian).

---

## Running

```bash
# Use the built-in synthetic drum break (no WAV needed)
cargo run

# Load your own WAV file
cargo run -- /path/to/break.wav
```

You can also drag and drop a WAV file onto the window at any time to hot-swap the sample without restarting.

---

## Interface

The window is split into four sections.

### Title bar

| Control | What it does |
|---------|--------------|
| **LOAD** | Open a file picker to load a WAV |
| **BPM** display + **–** / **+** | Decrease or increase tempo by 1 BPM |
| **PLAY / STOP** | Toggle the sequencer running |
| **REC** | Toggle record mode — pad triggers are quantised into the pattern while the sequencer plays |
| **CLR** | Erase all 16 pattern steps |
| **OCT** display + **DN** / **UP** | Shift the octave for pad triggers (–3 to +3). Affects keyboard and mouse pad triggers |
| **VU meter** | Peak output level; green → yellow → red |

---

### Sample Waveform

Shows the loaded sample as a downsampled amplitude view.

- **Vertical markers** are drawn at each slice boundary, labelled 00–15.
- The **selected slice** (last pad you hit) is highlighted in navy blue with a thicker line.
- When the sequencer plays a step, the corresponding slice region **flashes green** in the waveform.
- The **footer** shows the filename and the currently selected slice number.

---

### Pattern Editor

A 16-step grid split into two columns of 8. Each row shows:

```
## | SL | NOTE
01 | 00 | C-+0     ← step 1 plays slice 00 at concert pitch
02 | -- | ---      ← step 2 is empty
```

| Action | Result |
|--------|--------|
| **Left-click** an empty step | Fills it with the currently selected slice and octave |
| **Left-click** a filled step | Clears it |
| **Right-click** any step | Clears it |
| **Scroll wheel** on a filled step | Changes the pitch of that step in semitones (–24 to +24) |

The **current playing step** is outlined in navy and tinted gold. Filled steps are tinted green.

---

### Sample Pads

16 pads arranged in two rows of 8. Each pad shows its number and keyboard shortcut.

- **Click** a pad to trigger it immediately.
- Clicking also **selects** that slice, so subsequent pattern clicks use it.
- Pads flash blue when triggered.
- The **selected pad** has a blue border and a lighter blue background.

---

## Keyboard Shortcuts

### Pad triggers

```
Row 1:  1  2  3  4  5  6  7  8   → pads 01–08
Row 2:  Q  W  E  R  T  Y  U  I   → pads 09–16
```

### Transport & editing

| Key | Action |
|-----|--------|
| `Space` | Play / Stop |
| `Tab` | Record on / off |
| `↑` / `↓` | Octave up / down |
| `Backspace` or `Delete` | Clear pattern |

---

## Record mode

With **REC on** and the sequencer **playing**, hitting a pad (keyboard or mouse) records the note into the nearest step in the sequence. The step is quantised to whichever beat is closest at the moment you hit the pad. This lets you tap in a pattern in real time.

---

## Pitch shifting

Every step can play its slice at a different pitch. The engine does real-time speed-ratio resampling — going up 12 semitones doubles playback speed, down 12 halves it. Use the scroll wheel on a pattern row to tune individual steps, or change the global **OCT** before clicking pads to record pitched hits.

---

## Loading samples

Chopper accepts mono or stereo WAV files (integer or float, any bit depth). Stereo files are summed to mono on load. Slices are detected automatically via transient/onset detection — if fewer than four transients are found, the sample is divided evenly into 16 equal slices.

The **first 16 slices** are mapped to the 16 pads. Slices beyond 16 exist in the pattern editor (you can type a higher slice number directly if you edit `Cmd::SetStep` programmatically) but are not shown as pads.

---

## Optional: DOS pixel font

Place a copy of **PxPlus IBM VGA8** (or any 8×8 TrueType DOS font) at:

```
assets/PxPlus_IBM_VGA8.ttf
```

The font is auto-loaded on startup if the file exists. It is freely available from the [Ultimate Oldschool PC Font Pack](https://int10h.org/oldschool-pc-fonts/) (CC0 license).

---

## Tips

- **Scratch a break**: load a classic drum break WAV, let transient detection find the hits, then rearrange them in the pattern editor with different pitches to chop it up.
- **Tune a kick**: put the same slice on two steps and scroll-down the second one by –5 or –7 semitones for a pitched variation.
- **Layer hits**: the engine runs 16 simultaneous voices — you can stack the same step trigger against itself at different pitches across the pattern.
- **No WAV?** The default synth break is a 16-step procedural kick/snare/hat pattern at 165 BPM. Try it with record mode to replace steps in real time.
