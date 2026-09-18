//! Bounded, off-thread presentation. The world never sees interpolation or pixels.
use super::live::{Pose, Scene, View};
use base64::Engine;
use std::io::{self, Write};

pub fn image_id() -> u32 {
    0x4357_0000 ^ std::process::id()
}
pub fn clear(output: &mut impl Write) -> io::Result<()> {
    write!(output, "\x1b_Ga=d,d=I,i={},q=2;\x1b\\", image_id())
}

#[derive(Default)]
pub struct Renderer {
    dimensions: (usize, usize),
    background: Vec<u8>,
    appearance: super::appearance::Appearance,
}
impl Renderer {
    pub fn set_appearance(&mut self, appearance: &super::appearance::Appearance) {
        if self.appearance != *appearance {
            self.dimensions = (0, 0);
            self.appearance = appearance.clone();
        }
    }
    pub fn render(
        &mut self,
        pose: &Pose,
        previous: Option<&Scene>,
        mix: f64,
        view: &View,
        time: f64,
    ) -> io::Result<(Vec<u8>, Option<Vec<u8>>)> {
        let cols = usize::from(view.width.clamp(1, 512));
        let rows = usize::from(view.height.clamp(1, 256));
        let cw = if view.cell_width.is_finite() && view.cell_width >= 1.0 {
            view.cell_width.min(64.0)
        } else {
            8.0
        };
        let ch = if view.cell_height.is_finite() && view.cell_height >= 1.0 {
            view.cell_height.min(128.0)
        } else {
            16.0
        };
        let width = cols as f64 * cw;
        let height = rows as f64 * ch;
        let resolution = (960.0 / width).min(560.0 / height).min(1.0);
        let w = (width * resolution).round().clamp(1.0, 960.0) as usize;
        let h = (height * resolution).round().clamp(1.0, 560.0) as usize;
        if self.dimensions != (w, h) {
            self.dimensions = (w, h);
            self.background = vec![0; w * h * 3];
            for y in 0..h {
                for x in 0..w {
                    let light = (1.0
                        - (((x as f64 / w as f64 - 0.5) * 1.1).powi(2)
                            + (y as f64 / h as f64 * 0.7).powi(2))
                        .sqrt())
                    .clamp(0.0, 1.0);
                    let at = (y * w + x) * 3;
                    for k in 0..3 {
                        self.background[at + k] = (f64::from(self.appearance.background[k])
                            * (1.0 - light)
                            + f64::from(self.appearance.background_top[k]) * light)
                            as u8;
                    }
                }
            }
        }
        let mut rgb = if view.pixels {
            self.background.clone()
        } else {
            Vec::new()
        };
        let mut cells = vec![0u8; cols * rows];
        let val = |name: &str, default: f64| pose.state[name].as_f64().unwrap_or(default);
        let attention = val("attention", 0.0);
        let flip = val("flip", 1.0);
        let scale = (w as f64 * 0.52).min(h as f64 * 0.85) * (1.0 + attention * 0.07);
        let ox = w as f64 * (0.5 + val("roamX", 0.0) * 0.30);
        let oy = h as f64 * (0.47 + val("roamY", 0.0) * 0.30 + attention * 0.05);
        let colour = [pose.style.r, pose.style.g, pose.style.b];
        if view.pixels && self.appearance.environment {
            for x in 16..w.saturating_sub(16) {
                blend(&mut rgb, w, h, x as i32, 16, [45.0, 70.0, 81.0], 0.5);
            }
            for band in 0..6 {
                for x in 0..w {
                    let y = h as f64 * 0.85
                        + ((x as f64 / 110.0) + band as f64 + time * 0.18).sin() * 6.0
                        + band as f64 * 5.0;
                    blend(
                        &mut rgb,
                        w,
                        h,
                        x as i32,
                        y as i32,
                        [96.0, 163.0, 185.0],
                        0.025,
                    );
                }
            }
        }
        let radius = (w.min(h) as f64 * 0.00285).clamp(0.68, 1.55) * self.appearance.dot_scale;
        for (i, q) in pose.points.iter().enumerate() {
            let p = previous.and_then(|s| s.points.get(i));
            let xx = p.map_or(q[0], |p| p[0] + (q[0] - p[0]) * mix);
            let yy = p.map_or(q[1], |p| p[1] + (q[1] - p[1]) * mix);
            let x = ox + xx * scale * flip;
            let y = oy + yy * scale;
            let cx = (x / w as f64 * (cols * 2) as f64).floor() as i32;
            let cy = (y / h as f64 * (rows * 4) as f64).floor() as i32;
            if cx >= 0 && cy >= 0 && cx < (cols * 2) as i32 && cy < (rows * 4) as i32 {
                let bit = [[0, 3], [1, 4], [2, 5], [6, 7]][cy as usize % 4][cx as usize % 2];
                cells[(cy as usize / 4) * cols + cx as usize / 2] |= 1 << bit;
            }
            if !view.pixels {
                continue;
            }
            let depth = 0.65 + 0.35 * ((i * 37 % 101) as f64 / 100.0);
            let alpha = (pose.style.alpha * depth).clamp(0.0, 1.0);
            let r = radius * depth;
            let glow = if !pose.style.hollow && i % 5 == 0 {
                1.0 + 4.0 * self.appearance.glow
            } else {
                1.0
            };
            let reach = (r * glow + 1.0).ceil() as i32;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let px = x.floor() as i32 + dx;
                    let py = y.floor() as i32 + dy;
                    let dist =
                        ((px as f64 + 0.5 - x).powi(2) + (py as f64 + 0.5 - y).powi(2)).sqrt();
                    let coverage = if pose.style.hollow {
                        (0.7 - (dist - r).abs()).clamp(0.0, 1.0)
                    } else {
                        (r + 0.5 - dist).clamp(0.0, 1.0)
                    };
                    let a = alpha * coverage
                        + if glow > 1.0 {
                            (1.0 - dist / (r * 3.0)).max(0.0) * 0.035
                        } else {
                            0.0
                        };
                    if a > 0.0 {
                        blend(&mut rgb, w, h, px, py, colour, a)
                    }
                }
            }
        }
        let image = if view.pixels {
            let mut compressor =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            compressor.write_all(&rgb)?;
            let data = base64::engine::general_purpose::STANDARD.encode(compressor.finish()?);
            let mut protocol = Vec::with_capacity(data.len() + data.len() / 4096 * 32 + 160);
            for (index, chunk) in data.as_bytes().chunks(4096).enumerate() {
                let more = u8::from((index + 1) * 4096 < data.len());
                if index == 0 {
                    write!(
                        protocol,
                        "\x1b_Ga=T,t=d,f=24,o=z,s={w},v={h},i={},p=1,c={cols},r={rows},C=1,q=2,m={more};",
                        image_id()
                    )?;
                } else {
                    write!(protocol, "\x1b_Gm={more},q=2;")?;
                }
                protocol.extend_from_slice(chunk);
                protocol.extend_from_slice(b"\x1b\\");
            }
            Some(protocol)
        } else {
            None
        };
        Ok((cells, image))
    }
}
fn blend(rgb: &mut [u8], w: usize, h: usize, x: i32, y: i32, c: [f64; 3], a: f64) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        return;
    }
    let offset = (y as usize * w + x as usize) * 3;
    let a = a.clamp(0.0, 1.0);
    for k in 0..3 {
        rgb[offset + k] = (rgb[offset + k] as f64 * (1.0 - a) + c[k] * a).clamp(0.0, 255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::super::live::Style;
    use super::*;
    #[test]
    fn pixel_transport_is_bounded_chunked_rgb_and_braille_remains_available() {
        let pose = Pose {
            points: vec![[0.0, 0.0]; 980],
            style: Style {
                r: 120.,
                g: 210.,
                b: 230.,
                alpha: 0.7,
                hollow: false,
                channel: "reasoning".into(),
                arch: "gyre".into(),
            },
            state: serde_json::json!({}),
        };
        let mut renderer = Renderer::default();
        let view = View {
            width: 120,
            height: 35,
            pixels: true,
            ..View::default()
        };
        let (cells, image) = renderer.render(&pose, None, 1.0, &view, 0.0).unwrap();
        assert_eq!(cells.len(), 4200);
        assert!(cells.iter().any(|b| *b != 0));
        let image = String::from_utf8(image.unwrap()).unwrap();
        assert!(image.contains("a=T,t=d,f=24,o=z,s=960,v=560"));
        let mut encoded = String::new();
        for chunk in image.split("\x1b\\").filter(|s| !s.is_empty()) {
            let (_, data) = chunk.split_once(';').unwrap();
            assert!(data.len() <= 4096);
            encoded.push_str(data);
        }
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let mut decoder = flate2::read::ZlibDecoder::new(compressed.as_slice());
        let mut rgb = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut rgb).unwrap();
        assert_eq!(rgb.len(), 960 * 560 * 3);
        let (fallback, image) = renderer
            .render(
                &pose,
                None,
                1.0,
                &View {
                    pixels: false,
                    ..view
                },
                0.0,
            )
            .unwrap();
        assert_eq!(fallback, cells);
        assert!(image.is_none());
    }
}
