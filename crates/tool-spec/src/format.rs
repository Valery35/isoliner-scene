// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Подстановка в шаблон в стиле printf, как оператор `%` в Python.
//!
//! Поддерживается то, что пишут расчёты модуля: `%d`, `%s`, `%f`,
//! `%g` с точностью (`%.1f`, `%.3g`) и `%%`.

/// Аргумент шаблона.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Int(i64),
    Float(f64),
    Text(String),
}

impl From<i64> for Arg {
    fn from(v: i64) -> Self {
        Arg::Int(v)
    }
}
impl From<usize> for Arg {
    fn from(v: usize) -> Self {
        Arg::Int(v as i64)
    }
}
impl From<f64> for Arg {
    fn from(v: f64) -> Self {
        Arg::Float(v)
    }
}
impl From<&str> for Arg {
    fn from(v: &str) -> Self {
        Arg::Text(v.to_string())
    }
}
impl From<String> for Arg {
    fn from(v: String) -> Self {
        Arg::Text(v)
    }
}

fn as_f64(a: &Arg) -> f64 {
    match a {
        Arg::Int(i) => *i as f64,
        Arg::Float(f) => *f,
        Arg::Text(t) => t.parse().unwrap_or(f64::NAN),
    }
}

/// `%g` Python: `prec` значащих цифр, без хвостовых нулей,
/// экспонента при порядке меньше -4 или не меньше `prec`.
fn general(x: f64, prec: usize) -> String {
    let p = prec.max(1);
    if x == 0.0 || !x.is_finite() {
        return if x.is_nan() {
            "nan".into()
        } else if x.is_infinite() {
            if x > 0.0 { "inf".into() } else { "-inf".into() }
        } else {
            "0".into()
        };
    }
    let e = format!("{:.*e}", p - 1, x);
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s.to_string()
        }
    };
    if exp < -4 || exp >= p as i32 {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{}{:02}", strip(mant), sign, exp.abs())
    } else {
        let decimals = (p as i32 - 1 - exp).max(0) as usize;
        strip(&format!("{x:.decimals$}"))
    }
}

/// Подставить аргументы в шаблон. Лишний `%` без аргумента остаётся.
pub fn format(template: &str, args: &[Arg]) -> String {
    let mut out = String::new();
    let mut it = args.iter();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            out.push('%');
            continue;
        }
        // флаги и ширина, как у % в Python: %+.1f, %5d, %-8s, %05.1f
        let (mut plus, mut left, mut zero, mut space) = (false, false, false, false);
        while let Some(f) = chars.peek().copied().filter(|f| "+- 0#".contains(*f)) {
            match f {
                '+' => plus = true,
                '-' => left = true,
                '0' => zero = true,
                ' ' => space = true,
                _ => {}
            }
            chars.next();
        }
        let mut width_s = String::new();
        while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
            width_s.push(*d);
            chars.next();
        }
        let width: usize = width_s.parse().unwrap_or(0);
        let mut prec: Option<usize> = None;
        if chars.peek() == Some(&'.') {
            chars.next();
            let mut digits = String::new();
            while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                digits.push(*d);
                chars.next();
            }
            prec = digits.parse().ok();
        }
        let Some(conv) = chars.next() else {
            out.push('%');
            break;
        };
        let Some(arg) = it.next() else {
            out.push('%');
            out.push(conv);
            continue;
        };
        let num = matches!(conv, 'd' | 'i' | 'f' | 'g');
        let mut piece = match conv {
            'd' | 'i' => format!("{}", as_f64(arg).trunc() as i64),
            'f' => format!("{:.*}", prec.unwrap_or(6), as_f64(arg)),
            'g' => general(as_f64(arg), prec.unwrap_or(6)),
            's' => match arg {
                Arg::Text(t) => t.clone(),
                Arg::Int(i) => i.to_string(),
                Arg::Float(f) => general(*f, 12),
            },
            other => {
                out.push('%');
                out.push(other);
                continue;
            }
        };
        if num && !piece.starts_with('-') {
            if plus {
                piece.insert(0, '+');
            } else if space {
                piece.insert(0, ' ');
            }
        }
        let len = piece.chars().count();
        if len < width {
            let pad = width - len;
            if left {
                piece.push_str(&" ".repeat(pad));
            } else if zero && num {
                // нули после знака
                let sign = piece.starts_with(['+', '-', ' ']);
                let at = usize::from(sign);
                piece.insert_str(at, &"0".repeat(pad));
            } else {
                piece.insert_str(0, &" ".repeat(pad));
            }
        }
        out.push_str(&piece);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_python() {
        assert_eq!(
            format(
                "Колонок %d x %d, шаг %.3g м.",
                &[706usize.into(), 568usize.into(), 0.145.into()]
            ),
            "Колонок 706 x 568, шаг 0.145 м."
        );
        assert_eq!(format("%.1f %% тела", &[25.0.into()]), "25.0 % тела");
        assert_eq!(format("%.3g", &[1234.5.into()]), "1.23e+03");
        assert_eq!(format("%.3g", &[0.00001234.into()]), "1.23e-05");
        assert_eq!(format("%.3g", &[2.0.into()]), "2");
        assert_eq!(
            format("%.3f %s", &[(-299.25).into(), "ниже".into()]),
            "-299.250 ниже"
        );
        assert_eq!(format("нет %d", &[]), "нет %d");
        // флаги и ширина
        assert_eq!(format("%+.1f процента", &[3.25.into()]), "+3.2 процента");
        assert_eq!(format("%+.1f", &[(-7.0).into()]), "-7.0");
        assert_eq!(
            format(
                "[%5d|%-4s|%05.1f]",
                &[42i64.into(), "ab".into(), 3.5.into()]
            ),
            "[   42|ab  |003.5]"
        );
    }
}
