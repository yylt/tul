//! Unit conversion: length, weight, temperature, area, volume, speed,
//! data size, time. Pure Rust, no dependencies.

use wasm_bindgen::prelude::*;

/// Convert between units of a category.
///
/// Returns the converted value, or NaN when the value cannot be parsed
/// (JS side displays "invalid"). Throws only for unknown units.
#[wasm_bindgen]
pub fn convert_unit(category: &str, value: f64, from: &str, to: &str) -> Result<f64, JsValue> {
    convert_unit_impl(category, value, from, to).map_err(|e| JsValue::from_str(&e))
}

/// Internal implementation (testable on host, no wasm-bindgen).
pub(crate) fn convert_unit_impl(
    category: &str,
    value: f64,
    from: &str,
    to: &str,
) -> Result<f64, String> {
    let table: &[(&str, f64)] = match category {
        "length" => &[
            ("m", 1.0),
            ("km", 1000.0),
            ("cm", 0.01),
            ("mm", 0.001),
            ("in", 0.0254),
            ("ft", 0.3048),
            ("yd", 0.9144),
            ("mi", 1609.344),
        ],
        "weight" => &[
            ("kg", 1.0),
            ("g", 0.001),
            ("mg", 0.000001),
            ("t", 1000.0),
            ("lb", 0.45359237),
            ("oz", 0.028349523125),
        ],
        "area" => &[
            ("m2", 1.0),
            ("km2", 1000000.0),
            ("ha", 10000.0),
            ("acre", 4046.8564224),
            ("ft2", 0.09290304),
        ],
        "volume" => &[
            ("L", 1.0),
            ("mL", 0.001),
            ("m3", 1000.0),
            ("gal", 3.785411784),
        ],
        "speed" => &[
            ("kmh", 1.0),
            ("mph", 1.609344),
            ("ms", 3.6),
            ("knot", 1.852),
        ],
        "data" => &[
            ("B", 1.0),
            ("KB", 1024.0),
            ("MB", 1024.0 * 1024.0),
            ("GB", 1024.0 * 1024.0 * 1024.0),
            ("TB", 1024.0f64.powi(4)),
            ("PB", 1024.0f64.powi(5)),
        ],
        "time" => &[
            ("s", 1.0),
            ("min", 60.0),
            ("h", 3600.0),
            ("day", 86400.0),
            ("week", 604800.0),
            ("month", 2629800.0), // 30.4375 days
            ("year", 31557600.0), // 365.25 days
        ],
        "temperature" => &[("C", 0.0), ("F", 0.0), ("K", 0.0)], // handled by convert_temperature
        _ => return Err("unknown category".to_string()),
    };

    if value.is_nan() {
        return Ok(f64::NAN);
    }

    let (from_f, to_f) = if category == "temperature" {
        (0.0, 0.0) // handled separately below
    } else {
        let from_f = table
            .iter()
            .find(|(u, _)| *u == from)
            .ok_or_else(|| format!("unknown unit: {from}"))?;
        let to_f = table
            .iter()
            .find(|(u, _)| *u == to)
            .ok_or_else(|| format!("unknown unit: {to}"))?;
        (from_f.1, to_f.1)
    };

    if category == "temperature" {
        return Ok(convert_temperature(value, from, to));
    }

    Ok(value * from_f / to_f)
}

fn convert_temperature(value: f64, from: &str, to: &str) -> f64 {
    // normalise to Kelvin
    let k = match from {
        "C" => value + 273.15,
        "F" => (value - 32.0) * 5.0 / 9.0 + 273.15,
        "K" => value,
        _ => return f64::NAN,
    };
    match to {
        "C" => k - 273.15,
        "F" => (k - 273.15) * 9.0 / 5.0 + 32.0,
        "K" => k,
        _ => f64::NAN,
    }
}
