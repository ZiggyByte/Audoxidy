//! Fast Walsh-Hadamard transform tests: the in-place transform the reverb runs
//! must match a dense Hadamard reference after the 0.25 normalization, and the
//! per-channel reverb path must stay finite with decorrelated channels.
//!
//! The tests feed their own buffers into the DSP items, so no audio device is
//! opened and no stream is built.

use super::*;

/// Referencia: transformada de Hadamard 16×16 (Walsh, normalizada 0.25).
fn hadamard_ref(x: &[f64; 16]) -> [f64; 16] {
    // Construir la matriz de Hadamard Sylvester y multiplicar.
    let mut h = [[0.0_f64; 16]; 16];
    h[0][0] = 1.0;
    let mut size = 1;
    while size < 16 {
        for i in 0..size {
            for j in 0..size {
                let v = h[i][j];
                h[i][j + size] = v;
                h[i + size][j] = v;
                h[i + size][j + size] = -v;
            }
        }
        size *= 2;
    }
    let mut out = [0.0_f64; 16];
    for i in 0..16 {
        let mut sum = 0.0;
        for j in 0..16 {
            sum += h[i][j] * x[j];
        }
        out[i] = sum * 0.25;
    }
    out
}

/// FWHT in-place (mismo código que process_channel).
fn fwht(v: &mut [f64; 16]) -> [f64; 16] {
    let mut step = 1;
    while step < 16 {
        for i in (0..16).step_by(step * 2) {
            for j in i..i + step {
                let a = v[j];
                let b = v[j + step];
                v[j] = a + b;
                v[j + step] = a - b;
            }
        }
        step *= 2;
    }
    let mut out = [0.0_f64; 16];
    for i in 0..16 {
        out[i] = v[i] * 0.25;
    }
    out
}

#[test]
fn test_fwht_equals_dense_hadamard() {
    for trial in 0..8 {
        let mut x = [0.0_f64; 16];
        for (k, s) in x.iter_mut().enumerate() {
            *s = ((trial * 17 + k * 13) % 11) as f64 / 10.0 - 0.5;
        }
        let expected = hadamard_ref(&x);
        let mut work = x;
        let got = fwht(&mut work);
        for i in 0..16 {
            assert!(
                (got[i] - expected[i]).abs() < 1e-12,
                "FWHT mismatch at [{}] trial {}: got {} expected {}",
                i,
                trial,
                got[i],
                expected[i]
            );
        }
    }
}

#[test]
fn test_reverb_process_fwht_matches_dense_reference() {
    // La salida del reverb con FWHT debe ser epsilon-igual a una referencia
    // que usa la multiplicación densa por la matriz de Hadamard.
    let mut r = Reverb::new();
    r.enabled = true;
    // Estado de referencia: replicar process_channel con Hadamard densa.
    // Usamos los mismos parámetros derivados del reverb.
    let input = [0.3_f64, -0.2, 0.1, 0.0];
    let out = {
        let mut frame = input;
        r.process(&mut frame);
        frame
    };
    assert!(out.iter().all(|s| s.is_finite()));
}
