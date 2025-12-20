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
            let mut k = 0;
            while k < 1000 {
                z = z * z + c;
                if z.norm_sqr() > 4.0 {
                    break;
                }
                k += 1;
            }
            output.push(match k {
                0..=9 => '.',
                10..=19 => '*',
                20..=29 => 'o',
                30..=39 => 'O',
                _ => ' ',
            });
        }
        output.push('\n');
    }

    // Mirror on the y-axis.
    let lines: Vec<&str> = output.lines().collect();
    let mirrored: String = lines.iter()
        .rev()
        .skip(1)
        .map(|&line| format!("{}\n", line))
        .collect();
    output.push_str(&mirrored);

    Value::make_string(&output)
}
