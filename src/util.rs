use std::sync::Arc;

pub fn load_wav(path: &str) -> Option<(Arc<Vec<f32>>, u32)> {
    let mut r = hound::WavReader::open(path).ok()?;
    let spec = r.spec();
    let ch = spec.channels as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().filter_map(|s| s.ok()).map(|s| s as f32 / max).collect()
        }
    };
    let mono: Vec<f32> = if ch <= 1 {
        raw
    } else {
        raw.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect()
    };
    Some((Arc::new(mono), spec.sample_rate))
}

pub fn slice_even(len: usize, n: usize) -> Vec<(usize, usize)> {
    let w = len / n;
    (0..n).map(|i| (i * w, ((i + 1) * w).min(len))).collect()
}

pub fn slice_transient(s: &[f32], sr: f32) -> Vec<(usize, usize)> {
    let hop = 256usize;
    let frames = s.len() / hop;
    if frames < 4 {
        return slice_even(s.len(), 16);
    }

    let mut energy = vec![0.0f32; frames];
    for (f, e) in energy.iter_mut().enumerate() {
        let a = f * hop;
        let b = (a + hop).min(s.len());
        *e = s[a..b].iter().map(|x| x * x).sum::<f32>() / hop as f32;
    }

    let mut flux = vec![0.0f32; frames];
    for f in 1..frames {
        flux[f] = (energy[f] - energy[f - 1]).max(0.0);
    }

    let mean = flux.iter().sum::<f32>() / frames as f32;
    let thresh = mean * 3.0;
    let min_gap = ((sr * 0.04) / hop as f32) as usize;

    let mut onsets = vec![0usize];
    let mut last = 0usize;
    for f in 1..frames - 1 {
        if flux[f] > thresh
            && flux[f] >= flux[f - 1]
            && flux[f] >= flux[f + 1]
            && f - last >= min_gap
        {
            onsets.push(f * hop);
            last = f;
        }
    }

    if onsets.len() < 4 {
        return slice_even(s.len(), 16);
    }

    onsets
        .windows(2)
        .map(|w| (w[0], w[1]))
        .chain(std::iter::once((*onsets.last().unwrap(), s.len())))
        .collect()
}

pub fn synth_break(sr: f32) -> Arc<Vec<f32>> {
    let step = (sr * 60.0 / 165.0 / 4.0) as usize;
    let len = step * 16;
    let mut buf = vec![0.0f32; len];
    let mut seed = 0x9e3779b9u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let kick = |buf: &mut [f32], at: usize| {
        let dur = (sr * 0.18) as usize;
        for i in 0..dur {
            if at + i >= buf.len() {
                break;
            }
            let t = i as f32 / sr;
            let f = 120.0 * (-t * 18.0).exp() + 45.0;
            let env = (-t * 14.0).exp();
            buf[at + i] += (t * f * std::f32::consts::TAU).sin() * env * 0.9;
        }
    };
    let snare = |buf: &mut [f32], at: usize, n: &mut dyn FnMut() -> f32| {
        let dur = (sr * 0.16) as usize;
        for i in 0..dur {
            if at + i >= buf.len() {
                break;
            }
            let t = i as f32 / sr;
            let env = (-t * 22.0).exp();
            let tone = (t * 190.0 * std::f32::consts::TAU).sin() * 0.4;
            buf[at + i] += (n() * 0.6 + tone) * env * 0.7;
        }
    };
    let hat = |buf: &mut [f32], at: usize, n: &mut dyn FnMut() -> f32| {
        let dur = (sr * 0.04) as usize;
        for i in 0..dur {
            if at + i >= buf.len() {
                break;
            }
            let env = (-(i as f32 / sr) * 80.0).exp();
            buf[at + i] += n() * env * 0.35;
        }
    };
    for (i, &hit) in [1, 0, 2, 0, 3, 0, 1, 2, 0, 1, 0, 2, 3, 2, 1, 0]
        .iter()
        .enumerate()
    {
        let at = i * step;
        match hit {
            1 => kick(&mut buf, at),
            2 => snare(&mut buf, at, &mut noise),
            3 => kick(&mut buf, at),
            _ => {}
        }
        if i % 2 == 1 {
            hat(&mut buf, at, &mut noise);
        }
    }
    Arc::new(buf)
}
