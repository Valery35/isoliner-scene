// Isoliner Scene - сцена Isoliner3D без QGIS.
// Copyright (C) 2026 ООО «Информ++». Лицензия Apache 2.0.
// SPDX-License-Identifier: Apache-2.0

//! Отчёт по горизонтам блочной модели: руда, вскрыша, металл, деньги.
//! Перенос `benches.py` модуля.
//!
//! Модель режется горизонтальными уступами от базовой отметки. Блок,
//! который пересекает границу уступа, делится между уступами по доле
//! своей высоты. Блок без высоты целиком уходит в уступ своего центра.
//! Руда - блоки с содержанием не ниже бортового, блок без содержания -
//! вскрыша. Металл = руда * содержание * единица (0.01 для процентов,
//! 1 для г/т).

/// Больше нуля, как `x > 0` в Python: NaN - нет.
pub(crate) fn positive(x: f64) -> bool {
    x > 0.0
}

/// Единица содержания: проценты - металл в тоннах, г/т - в граммах.
pub const UNITS: [f64; 2] = [0.01, 1.0];

/// Строка отчёта: уступ или итог.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub z_top: f64,
    pub z_bot: f64,
    pub n_blocks: i64,
    pub vol: f64,
    pub ton: f64,
    pub ore_t: f64,
    pub waste_t: f64,
    /// Среднее содержание руды, None - руды нет.
    pub grade: Option<f64>,
    pub metal: f64,
    /// Коэффициент вскрыши, None - руды нет.
    pub strip: Option<f64>,
    pub revenue: f64,
    pub cost: f64,
    pub margin: f64,
}

/// Экономика отчёта.
#[derive(Debug, Clone, Copy)]
pub struct Money {
    pub unit: f64,
    pub price: f64,
    pub recovery: f64,
    pub mine_cost: f64,
    pub proc_cost: f64,
}

/// Границы уступов, накрывающие отметки lo..hi, сверху вниз.
pub fn bench_edges(lo: &[f64], hi: &[f64], height: f64, base: f64) -> Option<Vec<(f64, f64)>> {
    if !positive(height) {
        return None;
    }
    let min = lo
        .iter()
        .copied()
        .filter(|x| !x.is_nan())
        .fold(f64::INFINITY, f64::min);
    let max = hi
        .iter()
        .copied()
        .filter(|x| !x.is_nan())
        .fold(f64::NEG_INFINITY, f64::max);
    let k0 = ((min - base) / height + 1e-9).floor() as i64;
    let mut k1 = ((max - base) / height - 1e-9).ceil() as i64;
    if k1 <= k0 {
        k1 = k0 + 1;
    }
    Some(
        (k0..k1)
            .rev()
            .map(|k| (base + (k + 1) as f64 * height, base + k as f64 * height))
            .collect(),
    )
}

#[allow(clippy::too_many_arguments)]
fn row(top: f64, bot: f64, n: i64, vol: f64, ton: f64, ore_t: f64, metal: f64, m: &Money) -> Row {
    let waste = ton - ore_t;
    let revenue = metal * m.recovery * m.price;
    let cost = ton * m.mine_cost + ore_t * m.proc_cost;
    Row {
        z_top: top,
        z_bot: bot,
        n_blocks: n,
        vol,
        ton,
        ore_t,
        waste_t: waste,
        grade: (ore_t > 0.0).then(|| metal / (ore_t * m.unit)),
        metal,
        strip: (ore_t > 0.0).then(|| waste / ore_t),
        revenue,
        cost,
        margin: revenue - cost,
    }
}

/// Строки отчёта по уступам сверху вниз и итог. `z_lo`, `z_hi` - низ
/// и верх блока, `grade` - NaN, если содержания нет. None - высота
/// уступа не больше нуля.
#[allow(clippy::too_many_arguments)]
pub fn report(
    z_lo: &[f64],
    z_hi: &[f64],
    vol: &[f64],
    grade: &[f64],
    dens: &[f64],
    height: f64,
    base: f64,
    cutoff: f64,
    m: &Money,
) -> Option<(Vec<Row>, Row)> {
    let n = vol.len();
    let lo: Vec<f64> = (0..n).map(|i| z_lo[i].min(z_hi[i])).collect();
    let hi: Vec<f64> = (0..n).map(|i| z_lo[i].max(z_hi[i])).collect();
    let ton: Vec<f64> = (0..n).map(|i| vol[i] * dens[i]).collect();
    let is_ore: Vec<bool> = grade
        .iter()
        .map(|g| g.is_finite() && *g >= cutoff)
        .collect();
    let span: Vec<f64> = (0..n).map(|i| hi[i] - lo[i]).collect();
    let mut rows = Vec::new();
    for (top, bot) in bench_edges(&lo, &hi, height, base)? {
        let share: Vec<f64> = (0..n)
            .map(|i| {
                if span[i] <= 1e-9 {
                    f64::from(u8::from(lo[i] >= bot && lo[i] < top))
                } else {
                    (hi[i].min(top) - lo[i].max(bot)).max(0.0) / span[i]
                }
            })
            .collect();
        let sel = share.iter().filter(|s| **s > 0.0).count();
        if sel == 0 {
            continue;
        }
        let mut t_sum = 0.0;
        let mut ore_sum = 0.0;
        let mut metal = 0.0;
        let mut v_sum = 0.0;
        for i in 0..n {
            let t = ton[i] * share[i];
            t_sum += t;
            v_sum += vol[i] * share[i];
            if is_ore[i] {
                ore_sum += t;
                metal += t * grade[i];
            }
        }
        rows.push(row(
            top,
            bot,
            sel as i64,
            v_sum,
            t_sum,
            ore_sum,
            metal * m.unit,
            m,
        ));
    }
    let sum = |f: fn(&Row) -> f64| rows.iter().map(f).sum::<f64>();
    let tot = row(
        rows.first().map_or(0.0, |r| r.z_top),
        rows.last().map_or(0.0, |r| r.z_bot),
        n as i64,
        sum(|r| r.vol),
        sum(|r| r.ton),
        sum(|r| r.ore_t),
        sum(|r| r.metal),
        m,
    );
    Some((rows, tot))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Блок 10 м на уступах по 5 м отдаёт каждому половину.
    #[test]
    fn a_block_is_shared_by_height() {
        let m = Money {
            unit: 0.01,
            price: 0.0,
            recovery: 1.0,
            mine_cost: 0.0,
            proc_cost: 0.0,
        };
        let (rows, tot) =
            report(&[0.0], &[10.0], &[100.0], &[1.0], &[2.0], 5.0, 0.0, 0.5, &m).unwrap();
        assert_eq!(rows.len(), 2);
        assert!((rows[0].ton - 100.0).abs() < 1e-12 && (rows[1].ton - 100.0).abs() < 1e-12);
        assert!((tot.metal - 2.0).abs() < 1e-12);
        assert!(report(&[0.0], &[1.0], &[1.0], &[1.0], &[1.0], 0.0, 0.0, 0.0, &m).is_none());
    }
}
