use serde::{Deserialize, Serialize};

/// Position is a percentage of the available travel on each axis.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Framing {
    pub zoom: u16,
    pub x: u16,
    pub y: u16,
    pub brightness: u16,
    pub speed: u16,
}

impl Default for Framing {
    fn default() -> Self {
        Self {
            zoom: 100,
            x: 50,
            y: 50,
            brightness: 100,
            speed: 100,
        }
    }
}

impl Framing {
    pub fn valid(self) -> bool {
        (100..=400).contains(&self.zoom)
            && self.x <= 100
            && self.y <= 100
            && self.brightness <= 150
            && (25..=400).contains(&self.speed)
    }

    pub fn bounds(self, fit: bool, src: (f64, f64), dst: (f64, f64)) -> (f64, f64, f64, f64) {
        let sx = dst.0 / src.0.max(1.);
        let sy = dst.1 / src.1.max(1.);
        let scale = (if fit { sx.min(sy) } else { sx.max(sy) }) * f64::from(self.zoom) / 100.;
        let (w, h) = (src.0 * scale, src.1 * scale);
        (
            (dst.0 - w) * f64::from(self.x) / 100.,
            (dst.1 - h) * f64::from(self.y) / 100.,
            w,
            h,
        )
    }

    pub fn brighten(self, pixels: &mut [u8], channels: usize) {
        if self.brightness == 100 {
            return;
        }
        let lut: [u8; 256] = std::array::from_fn(|i| ((i * usize::from(self.brightness) + 50) / 100).min(255) as u8);
        for pixel in pixels.chunks_exact_mut(channels) {
            for c in &mut pixel[..3] {
                *c = lut[usize::from(*c)];
            }
        }
    }
}
