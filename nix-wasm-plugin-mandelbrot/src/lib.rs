use nix_wasm_rust::Value;
use num::complex::Complex64;

const MIN_R: f64 = -2.05;
const MAX_R: f64 = 0.5;
const MIN_I: f64 = 0.0;
const MAX_I: f64 = 1.15;

#[no_mangle]
pub extern "C" fn mandelbrot(args: Value) -> Value {
    let width = args
        .get_attr("width")
        .map(|v| v.get_int() as usize)
        .unwrap_or(120);

    let height = ((MAX_I - MIN_I) / (MAX_R - MIN_R) * 0.6 * width as f64) as usize;

    let mut output = String::new();

    for r in 0..height {
        let ci = MIN_I + (MAX_I - MIN_I) * (height - r) as f64 / (height as f64);
        for i in 0..width {
            let cr = MIN_R + (MAX_R - MIN_R) * i as f64 / (width as f64);
            let c = Complex64::new(cr, ci);
            let mut z = Complex64::new(0.0, 0.0);
            let mut k: u32 = 0;
            while k < ITERATIONS {
                z = z * z + c;
                if z.norm_sqr() > 4.0 {
                    break;
                }
                k += 1;
            }

            // Smooth out the iteration count.
            let k_smooth = if k < ITERATIONS {
                let log_zn = (z.re * z.re + z.im * z.im).ln() / 2.0;
                let nu = (log_zn / std::f64::consts::LN_2).ln() / std::f64::consts::LN_2;
                k as f64 + 1.0 - nu
            } else {
                k as f64
            };

            let (r1, g1, b1) = iter_to_color(k);
            let (r2, g2, b2) = iter_to_color((k_smooth + 1.0).floor() as u32);
            let k_frac = k_smooth.fract();
            output.push_str(&format!(
                "\x1b[48;2;{};{};{}m ",
                to_rgb(interpolate(r1, r2, k_frac)),
                to_rgb(interpolate(g1, g2, k_frac)),
                to_rgb(interpolate(b1, b2, k_frac))
            ));
        }
        output.push_str("\x1b[0m\n"); // Reset color at end of line
    }

    // Mirror on the y-axis.
    let lines: Vec<&str> = output.lines().collect();
    let mirrored: String = lines
        .iter()
        .rev()
        .skip(1)
        .map(|&line| format!("{}\n", line))
        .collect();
    output.push_str(&mirrored);

    Value::make_string(&output)
}

const ITERATIONS: u32 = 1000;

fn iter_to_color(k: u32) -> (f64, f64, f64) {
    if k >= ITERATIONS {
        (0.0, 0.0, 0.0) // Black for points in the set
    } else {
        let t = (k as f64 / ITERATIONS as f64).powf(0.45);
        let r = (t * 4.0).clamp(0.0, 1.0);
        let g = (t * 2.0).clamp(0.0, 1.0);
        let b = (1.0 - (t - 0.25) * 4.0).clamp(0.0, 1.0);
        (r, g, b)
    }
}

fn to_rgb(x: f64) -> u8 {
    (x * 255.0) as u8
}

fn interpolate(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
