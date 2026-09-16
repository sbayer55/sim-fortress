//! Seeded lattice value noise and fractal sums used by world generation.
//!
//! Every lattice is filled from the generation `Rng`, so a seed fixes the
//! whole field; sampling is pure and never draws.

use crate::sim::rng::Rng;

/// Smooth value noise on a coarse lattice covering a `w` x `h` sample space
/// (callers pass the sample space, not the cell grid, so any aspect correction
/// must be applied before calling `new`).
pub(super) struct Noise {
    lattice: Vec<f32>,
    lw: usize,
    lh: usize,
    scale: f32,
}

impl Noise {
    pub(super) fn new(rng: &mut Rng, scale: f32, w: f32, h: f32) -> Self {
        let lw = crate::cast!((w / scale).ceil() => usize) + 2;
        let lh = crate::cast!((h / scale).ceil() => usize) + 2;
        let lattice = (0..lw * lh).map(|_| rng.f32()).collect();
        Self { lattice, lw, lh, scale }
    }

    /// Sample at (x, y); coordinates outside the sample space clamp to the edge.
    pub(super) fn at(&self, x: f32, y: f32) -> f32 {
        let fx = (x / self.scale).max(0.0);
        let fy = (y / self.scale).max(0.0);
        let x0 = crate::cast!(fx.floor() => usize);
        let y0 = crate::cast!(fy.floor() => usize);
        let tx = smooth(fx - crate::cast!(x0 => f32));
        let ty = smooth(fy - crate::cast!(y0 => f32));
        let g = |x: usize, y: usize| self.lattice[(y.min(self.lh - 1)) * self.lw + x.min(self.lw - 1)];
        let a = g(x0, y0) + (g(x0 + 1, y0) - g(x0, y0)) * tx;
        let b = g(x0, y0 + 1) + (g(x0 + 1, y0 + 1) - g(x0, y0 + 1)) * tx;
        a + (b - a) * ty
    }
}

/// Fractal Brownian motion: `octaves` layers of value noise, each at half the
/// scale and half the amplitude of the last, normalised to [0, 1].
pub(super) struct Fbm {
    octaves: Vec<Noise>,
    norm: f32,
}

impl Fbm {
    pub(super) fn new(rng: &mut Rng, base_scale: f32, octaves: usize, w: f32, h: f32) -> Self {
        let mut layers = Vec::with_capacity(octaves);
        let mut scale = base_scale;
        let mut amp = 1.0f32;
        let mut norm = 0.0f32;
        for _ in 0..octaves {
            layers.push(Noise::new(rng, scale.max(1.5), w, h));
            norm += amp;
            scale *= 0.5;
            amp *= 0.5;
        }
        Self { octaves: layers, norm }
    }

    pub(super) fn at(&self, x: f32, y: f32) -> f32 {
        let mut sum = 0.0f32;
        let mut amp = 1.0f32;
        for n in &self.octaves {
            sum += n.at(x, y) * amp;
            amp *= 0.5;
        }
        sum / self.norm
    }

    /// Ridged variant: each octave is folded about its midline so the field
    /// peaks along sharp crests (mountain chains) instead of round blobs.
    pub(super) fn ridged_at(&self, x: f32, y: f32) -> f32 {
        let mut sum = 0.0f32;
        let mut amp = 1.0f32;
        for n in &self.octaves {
            let v = 1.0 - (n.at(x, y) * 2.0 - 1.0).abs();
            sum += v * v * amp;
            amp *= 0.5;
        }
        sum / self.norm
    }
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
