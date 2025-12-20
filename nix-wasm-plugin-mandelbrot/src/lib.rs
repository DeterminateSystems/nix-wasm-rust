use nix_wasm_rust::Value;

#[no_mangle]
pub extern "C" fn mandelbrot(_arg: Value) -> Value {
    const MIN_R: f64 = -2.1;
    const MAX_R: f64 = 0.6;
    const MIN_I: f64 = 0.0;
    const MAX_I: f64 = 1.2;

    const WIDTH: usize = 120;
    const HEIGHT: usize = ((MAX_I - MIN_I) / (MAX_R - MIN_R) * 0.6 * WIDTH as f64) as usize;

    let mut result = [[' '; WIDTH]; HEIGHT];

    for r in 0..HEIGHT {
        let ci = MIN_I + (MAX_I - MIN_I) * r as f64 / (HEIGHT as f64);
        for i in 0..WIDTH {
            let cr = MIN_R + (MAX_R - MIN_R) * i as f64 / (WIDTH as f64);
            let mut zr = 0.0;
            let mut zi = 0.0;
            let mut k = 0;
            while k < 1000 {
                let tmp = zr * zr - zi * zi + cr;
                zi = 2.0 * zr * zi + ci;
                zr = tmp;
                if zr * zr + zi * zi > 4.0 {
                    break;
                }
                k += 1;
            }
            result[r][i] = match k {
                0..=9 => '.',
                10..=19 => '*',
                20..=29 => 'o',
                30..=39 => 'O',
                _ => ' ',
            };
        }
    }

    let rows: Vec<String> = result
        .iter()
        .rev()
        .map(|row| row.iter().collect::<String>())
        .collect();

    // Mirror around the imaginary axis (real axis in the plot)
    let mut mirrored_rows = rows.clone();
    mirrored_rows.extend(rows.iter().rev().skip(1).cloned());

    let output = mirrored_rows.join("\n");

    Value::make_string(&output)
}
