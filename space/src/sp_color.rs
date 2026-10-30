//! Port of `SPColor` and `SPGradient`.

use crate::libspace::html_to_color;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SPColor {
    r: i16,
    g: i16,
    b: i16,
    a: i16,
}

fn rgb_to_hex(r: i16, g: i16, b: i16) -> String {
    format!("#{:02X}{:02X}{:02X}", r as u8, g as u8, b as u8)
}

impl SPColor {
    pub const fn new(r: i16, g: i16, b: i16, a: i16) -> Self {
        Self { r, g, b, a }
    }

    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa`. Colors without an alpha
    /// component are opaque.
    pub fn from_html(html: &str) -> Self {
        let hex = html.strip_prefix('#').unwrap_or(html);
        if hex.len() == 3 {
            let expanded: String = hex.chars().flat_map(|c| [c, c]).collect();
            return Self::from_html(&format!("#{expanded}"));
        }
        let color = html_to_color(html);
        if hex.len() > 6 {
            Self::new(
                ((color >> 24) & 0xff) as i16,
                ((color >> 16) & 0xff) as i16,
                ((color >> 8) & 0xff) as i16,
                (color & 0xff) as i16,
            )
        } else {
            Self::new(
                ((color >> 16) & 0xff) as i16,
                ((color >> 8) & 0xff) as i16,
                (color & 0xff) as i16,
                255,
            )
        }
    }

    pub fn get_type(&self) -> &'static str {
        "Color"
    }

    pub fn get_r(&self) -> i16 {
        self.r
    }
    pub fn get_g(&self) -> i16 {
        self.g
    }
    pub fn get_b(&self) -> i16 {
        self.b
    }
    pub fn get_a(&self) -> i16 {
        self.a
    }
    pub fn set_r(&mut self, r: i16) {
        self.r = r;
    }
    pub fn set_g(&mut self, g: i16) {
        self.g = g;
    }
    pub fn set_b(&mut self, b: i16) {
        self.b = b;
    }
    pub fn set_a(&mut self, a: i16) {
        self.a = a;
    }

    pub fn is_transparent(&self) -> bool {
        self.a == 0
    }

    pub fn is_opaque(&self) -> bool {
        self.a == 255
    }

    /// All channels zero: "no color set".
    pub fn empty(&self) -> bool {
        self.a == 0 && self.r == 0 && self.g == 0 && self.b == 0
    }

    /// The same color with alpha `amount * 255` (so `alpha(1)` is opaque).
    pub fn alpha(&self, amount: f32) -> Self {
        Self::new(self.r, self.g, self.b, (amount * 255.0) as i16)
    }

    /// Scales each channel by `1 + luminance`, clamped to 0..255.
    pub fn lighten(&self, luminance: f32) -> Self {
        let channel = |v: i16| ((v as f32 + v as f32 * luminance) as i32).clamp(0, 255) as i16;
        Self::new(channel(self.r), channel(self.g), channel(self.b), self.a)
    }

    pub fn tint(&self, tint: &SPColor) -> Self {
        let ta = tint.a as f64 / 255.0;
        let channel = |v: i16, t: i16| (v as f64 + ((255 - v) as f64 * (t as f64 / 255.0)) * ta) as i16;
        Self::new(
            channel(self.r, tint.r),
            channel(self.g, tint.g),
            channel(self.b, tint.b),
            channel(self.a, tint.a),
        )
    }

    pub fn mix(&self, other: &SPColor, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let channel = |a: i16, b: i16| (a as f32 + (b - a) as f32 * t) as i16;
        Self::new(
            channel(self.r, other.r),
            channel(self.g, other.g),
            channel(self.b, other.b),
            channel(self.a, other.a),
        )
    }

    pub fn to_html_color(&self) -> String {
        rgb_to_hex(self.r, self.g, self.b)
    }

    /// HSL factory: h=0-360, s=0-1, l=0-1, a=0-255.
    pub fn from_hsl(h: f32, s: f32, l: f32, a: i16) -> Self {
        let mut h = h;
        while h < 0.0 {
            h += 360.0;
        }
        while h >= 360.0 {
            h -= 360.0;
        }
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);

        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let mut hh = h / 60.0;
        if hh >= 6.0 {
            hh = 0.0;
        }
        let x = c * (1.0 - ((hh % 2.0) - 1.0).abs());
        let m = l - c / 2.0;
        let (r1, g1, b1) = if hh < 1.0 {
            (c, x, 0.0)
        } else if hh < 2.0 {
            (x, c, 0.0)
        } else if hh < 3.0 {
            (0.0, c, x)
        } else if hh < 4.0 {
            (0.0, x, c)
        } else if hh < 5.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };
        let clamp = |v: f32| ((v * 255.0 + 0.5) as i32).clamp(0, 255) as i16;
        Self::new(clamp(r1 + m), clamp(g1 + m), clamp(b1 + m), a)
    }

    /// Extracts HSL components (h=0-360, s=0-1, l=0-1).
    pub fn to_hsl(&self) -> (f32, f32, f32) {
        let rf = self.r as f32 / 255.0;
        let gf = self.g as f32 / 255.0;
        let bf = self.b as f32 / 255.0;
        let mx = rf.max(gf).max(bf);
        let mn = rf.min(gf).min(bf);
        let delta = mx - mn;
        let l = (mx + mn) / 2.0;
        if delta < 0.0001 {
            return (0.0, 0.0, l);
        }
        let s = delta / (1.0 - (2.0 * l - 1.0).abs());
        let mut h = if mx == rf {
            60.0 * (((gf - bf) / delta) % 6.0)
        } else if mx == gf {
            60.0 * ((bf - rf) / delta + 2.0)
        } else {
            60.0 * ((rf - gf) / delta + 4.0)
        };
        if h < 0.0 {
            h += 360.0;
        }
        (h, s, l)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SPGradient {
    direction: i16,
    start: SPColor,
    end: SPColor,
}

impl SPGradient {
    pub const fn new(start: SPColor, end: SPColor, direction: i16) -> Self {
        Self { direction, start, end }
    }
    pub fn get_start(&self) -> SPColor {
        self.start
    }
    pub fn set_start(&mut self, value: SPColor) {
        self.start = value;
    }
    pub fn get_end(&self) -> SPColor {
        self.end
    }
    pub fn set_end(&mut self, value: SPColor) {
        self.end = value;
    }
    /// 0 = vertical (top to bottom), 1 = horizontal (left to right).
    pub fn get_direction(&self) -> i16 {
        self.direction
    }
    pub fn set_direction(&mut self, value: i16) {
        self.direction = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_html_colors() {
        assert_eq!(SPColor::from_html("#ff8000"), SPColor::new(255, 128, 0, 255));
        assert_eq!(SPColor::from_html("#f80"), SPColor::new(255, 136, 0, 255));
        assert_eq!(SPColor::from_html("#ff800080"), SPColor::new(255, 128, 0, 128));
        assert_eq!(SPColor::new(255, 128, 0, 255).to_html_color(), "#FF8000");
    }

    #[test]
    fn hsl_round_trip() {
        let color = SPColor::from_hsl(200.0, 0.7, 0.5, 255);
        let (h, s, l) = color.to_hsl();
        assert!((h - 200.0).abs() < 1.0);
        assert!((s - 0.7).abs() < 0.02);
        assert!((l - 0.5).abs() < 0.02);
    }

    #[test]
    fn lighten_and_alpha() {
        let c = SPColor::new(100, 100, 100, 255);
        assert_eq!(c.lighten(0.5).get_r(), 150);
        assert_eq!(c.lighten(10.0).get_r(), 255);
        assert_eq!(c.alpha(0.0).get_a(), 0);
        assert_eq!(c.alpha(1.0).get_a(), 255);
    }
}
