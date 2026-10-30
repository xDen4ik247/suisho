//! Unit conversion: "3 km/h to m/s", "100 °F to °C", "1 atm to Pa", "c to km/s", "5 kWh to J" (also "в" for RU).
//! Units are products/quotients/powers of named units with SI prefixes, reduced to SI base dimensions
//! (m, kg, s, A, K, mol, cd); temperatures convert affinely when they stand alone.

type Dim = [i8; 7];

struct U {
    f: f64,
    d: Dim,
    off: f64, // affine offset (temperatures): SI = value·f + off
}

fn base(name: &str) -> Option<U> {
    let d = |m, kg, s, a, k, mol, cd| [m, kg, s, a, k, mol, cd];
    let u = |f: f64, dim: Dim| U { f, d: dim, off: 0.0 };
    Some(match name {
        "m" => u(1.0, d(1, 0, 0, 0, 0, 0, 0)),
        "g" => u(1e-3, d(0, 1, 0, 0, 0, 0, 0)),
        "s" | "sec" => u(1.0, d(0, 0, 1, 0, 0, 0, 0)),
        "A" => u(1.0, d(0, 0, 0, 1, 0, 0, 0)),
        "K" => u(1.0, d(0, 0, 0, 0, 1, 0, 0)),
        "mol" => u(1.0, d(0, 0, 0, 0, 0, 1, 0)),
        "cd" => u(1.0, d(0, 0, 0, 0, 0, 0, 1)),
        "min" => u(60.0, d(0, 0, 1, 0, 0, 0, 0)),
        "h" | "hr" => u(3600.0, d(0, 0, 1, 0, 0, 0, 0)),
        "day" | "d" => u(86400.0, d(0, 0, 1, 0, 0, 0, 0)),
        "week" => u(604800.0, d(0, 0, 1, 0, 0, 0, 0)),
        "year" | "yr" => u(31557600.0, d(0, 0, 1, 0, 0, 0, 0)),
        "Hz" => u(1.0, d(0, 0, -1, 0, 0, 0, 0)),
        "N" => u(1.0, d(1, 1, -2, 0, 0, 0, 0)),
        "J" => u(1.0, d(2, 1, -2, 0, 0, 0, 0)),
        "W" => u(1.0, d(2, 1, -3, 0, 0, 0, 0)),
        "Pa" => u(1.0, d(-1, 1, -2, 0, 0, 0, 0)),
        "bar" => u(1e5, d(-1, 1, -2, 0, 0, 0, 0)),
        "atm" => u(101325.0, d(-1, 1, -2, 0, 0, 0, 0)),
        "mmHg" => u(133.322387415, d(-1, 1, -2, 0, 0, 0, 0)),
        "psi" => u(6894.757293168, d(-1, 1, -2, 0, 0, 0, 0)),
        "C" => u(1.0, d(0, 0, 1, 1, 0, 0, 0)),
        "V" => u(1.0, d(2, 1, -3, -1, 0, 0, 0)),
        "ohm" | "Ω" => u(1.0, d(2, 1, -3, -2, 0, 0, 0)),
        "F" => u(1.0, d(-2, -1, 4, 2, 0, 0, 0)),
        "T" => u(1.0, d(0, 1, -2, -1, 0, 0, 0)),
        "Wb" => u(1.0, d(2, 1, -2, -1, 0, 0, 0)),
        "L" | "l" => u(1e-3, d(3, 0, 0, 0, 0, 0, 0)),
        "eV" => u(1.602176634e-19, d(2, 1, -2, 0, 0, 0, 0)),
        "cal" => u(4.184, d(2, 1, -2, 0, 0, 0, 0)),
        "Wh" => u(3600.0, d(2, 1, -2, 0, 0, 0, 0)),
        "hp" => u(745.69987158227, d(2, 1, -3, 0, 0, 0, 0)),
        "in" | "inch" => u(0.0254, d(1, 0, 0, 0, 0, 0, 0)),
        "ft" => u(0.3048, d(1, 0, 0, 0, 0, 0, 0)),
        "yd" => u(0.9144, d(1, 0, 0, 0, 0, 0, 0)),
        "mi" | "mile" => u(1609.344, d(1, 0, 0, 0, 0, 0, 0)),
        "nmi" => u(1852.0, d(1, 0, 0, 0, 0, 0, 0)),
        "au" | "AU" => u(149597870700.0, d(1, 0, 0, 0, 0, 0, 0)),
        "ly" => u(9.4607304725808e15, d(1, 0, 0, 0, 0, 0, 0)),
        "pc" => u(3.0856775814913673e16, d(1, 0, 0, 0, 0, 0, 0)),
        "lb" => u(0.45359237, d(0, 1, 0, 0, 0, 0, 0)),
        "oz" => u(0.028349523125, d(0, 1, 0, 0, 0, 0, 0)),
        "t" | "ton" => u(1000.0, d(0, 1, 0, 0, 0, 0, 0)),
        "kn" | "knot" => u(1852.0 / 3600.0, d(1, 0, -1, 0, 0, 0, 0)),
        "mph" => u(1609.344 / 3600.0, d(1, 0, -1, 0, 0, 0, 0)),
        "ha" => u(1e4, d(2, 0, 0, 0, 0, 0, 0)),
        "deg" | "°" => u(std::f64::consts::PI / 180.0, d(0, 0, 0, 0, 0, 0, 0)),
        "rad" => u(1.0, d(0, 0, 0, 0, 0, 0, 0)),
        "°C" | "degC" => U { f: 1.0, d: d(0, 0, 0, 0, 1, 0, 0), off: 273.15 },
        "°F" | "degF" => U { f: 5.0 / 9.0, d: d(0, 0, 0, 0, 1, 0, 0), off: 273.15 - 32.0 * 5.0 / 9.0 },
        // physical constants (as quantities)
        "c" => u(299792458.0, d(1, 0, -1, 0, 0, 0, 0)),
        "G" => u(6.67430e-11, d(3, -1, -2, 0, 0, 0, 0)),
        "gn" | "g0" => u(9.80665, d(1, 0, -2, 0, 0, 0, 0)),
        "hbar" | "ħ" => u(1.054571817e-34, d(2, 1, -1, 0, 0, 0, 0)),
        "planck" => u(6.62607015e-34, d(2, 1, -1, 0, 0, 0, 0)),
        "kB" => u(1.380649e-23, d(2, 1, -2, 0, -1, 0, 0)),
        "NA" => u(6.02214076e23, d(0, 0, 0, 0, 0, -1, 0)),
        "qe" => u(1.602176634e-19, d(0, 0, 1, 1, 0, 0, 0)),
        "me" => u(9.1093837015e-31, d(0, 1, 0, 0, 0, 0, 0)),
        "mp" => u(1.67262192369e-27, d(0, 1, 0, 0, 0, 0, 0)),
        "Rgas" => u(8.314462618, d(2, 1, -2, 0, -1, -1, 0)),
        _ => return None,
    })
}

fn prefixed(name: &str) -> Option<U> {
    if let Some(u) = base(name) {
        return Some(u);
    }
    let prefixes: [(&str, f64); 14] = [("Y", 1e24), ("Z", 1e21), ("E", 1e18), ("P", 1e15), ("T", 1e12), ("G", 1e9), ("M", 1e6), ("k", 1e3),
                                       ("h", 1e2), ("c", 1e-2), ("m", 1e-3), ("µ", 1e-6), ("u", 1e-6), ("n", 1e-9)];
    for (p, f) in prefixes.iter().chain([("p", 1e-12), ("da", 1e1)].iter()) {
        if let Some(rest) = name.strip_prefix(p) {
            if let Some(u) = base(rest) {
                if u.off != 0.0 {
                    continue;
                }
                return Some(U { f: u.f * f, d: u.d, off: 0.0 });
            }
        }
    }
    None
}

/// "km/h", "m/s^2", "kg*m^2/s^2", "N·m" → (factor, dimension, offset)
fn unit_expr(s: &str) -> Option<U> {
    let s = s.trim().replace('·', "*").replace('²', "^2").replace('³', "^3");
    if s.is_empty() {
        return None;
    }
    let (num, den) = match s.split_once('/') {
        Some((a, b)) => (a.to_string(), Some(b.to_string())),
        None => (s.clone(), None),
    };
    let mut f = 1.0;
    let mut d: Dim = [0; 7];
    let mut parts = 0;
    let mut single_off = 0.0;
    for (sign, chunk) in [(1i8, Some(num)), (-1i8, den)] {
        let Some(chunk) = chunk else { continue };
        for tok in chunk.split(['*', ' ']).filter(|x| !x.is_empty()) {
            let (name, pow) = match tok.split_once('^') {
                Some((n, p)) => (n, p.parse::<i8>().ok()?),
                None => (tok, 1),
            };
            let u = prefixed(name)?;
            parts += 1;
            single_off = u.off;
            f *= u.f.powi((sign * pow) as i32);
            for i in 0..7 {
                d[i] += u.d[i] * sign * pow;
            }
        }
    }
    Some(U { f, d, off: if parts == 1 { single_off } else { 0.0 } })
}

/// Parse and convert a whole row. None when the row is not a conversion.
pub fn convert(text: &str) -> Option<Result<(f64, String, String), String>> {
    let t = text.trim();
    let (lhs, to) = t.split_once(" to ").or_else(|| t.split_once(" в ")).or_else(|| t.split_once(" in "))?;
    let lhs = lhs.trim();
    // the leading number (may be absent: "c to km/s")
    let split = lhs.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E')).unwrap_or(lhs.len());
    let (num_s, unit_s) = lhs.split_at(split);
    let (num_s, unit_s) = if num_s.ends_with(['e', 'E']) { (&num_s[..num_s.len() - 1], &lhs[split - 1..]) } else { (num_s, unit_s) };
    let v: f64 = if num_s.trim().is_empty() { 1.0 } else { num_s.trim().parse().ok()? };
    let from = unit_expr(unit_s)?;
    let Some(dest) = unit_expr(to) else {
        return Some(Err(format!("unknown unit “{}”", to.trim())));
    };
    if from.d != dest.d {
        return Some(Err("the units measure different things (dimensions differ)".into()));
    }
    let si = v * from.f + from.off;
    let out = (si - dest.off) / dest.f;
    Some(Ok((out, unit_s.trim().to_string(), to.trim().to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(s: &str) -> f64 {
        convert(s).unwrap().unwrap().0
    }
    #[test]
    fn conversions() {
        assert!((c("36 km/h to m/s") - 10.0).abs() < 1e-12);
        assert!((c("100 °F to °C") - 37.777777777).abs() < 1e-6);
        assert!((c("0 °C to K") - 273.15).abs() < 1e-9);
        assert!((c("1 atm to Pa") - 101325.0).abs() < 1e-9);
        assert!((c("1 kWh to J") - 3.6e6).abs() < 1e-6);
        assert!((c("c to km/s") - 299792.458).abs() < 1e-6);
        assert!((c("1 mi to km") - 1.609344).abs() < 1e-12);
        assert!(convert("3 kg to m").unwrap().is_err());
        assert!(convert("x^2 + 1").is_none());
    }
}
