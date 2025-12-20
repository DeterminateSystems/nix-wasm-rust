use nix_wasm_rust::Value;
use num::complex::Complex64;

#[no_mangle]
pub extern "C" fn mandelbrot(_arg: Value) -> Value {
    const MIN_R: f64 = -2.05;
    const MAX_R: f64 = 0.5;
    const MIN_I: f64 = 0.0;
    const MAX_I: f64 = 1.15;

    const WIDTH: usize = 120;
    const HEIGHT: usize = ((MAX_I - MIN_I) / (MAX_R - MIN_R) * 0.6 * WIDTH as f64) as usize;

    let mut output = String::new();

    for r in 0..HEIGHT {
        let ci = MIN_I + (MAX_I - MIN_I) * (HEIGHT - r - 1) as f64 / (HEIGHT as f64);
        for i in 0..WIDTH {
            let cr = MIN_R + (MAX_R - MIN_R) * i as f64 / (WIDTH as f64);
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
            let (r, g, b) = iter_to_color(k);
            output.push_str(&format!("\x1b[48;2;{};{};{}m ", r, g, b));
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

fn iter_to_color(k: u32) -> (u8, u8, u8) {
    if k >= 1000 {
        (0, 0, 0) // Black for points in the set
    } else {
        let t = (k as f64 / ITERATIONS as f64).powf(0.45);
        let r = (t * 2.0).clamp(0.0, 1.0);
        let g = if t < 0.75 {
            (t * 4.0).min(1.0)
        } else {
            1.0 - (t - 0.75) * 4.0
        };
        let b = (1.0 - (t - 0.25) * 4.0).clamp(0.0, 1.0);
        ((g * 255.0) as u8, (r * 255.0) as u8, (b * 255.0) as u8)
    }
}
