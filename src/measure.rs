use std::fs;
use std::time::{Duration, Instant};

const CLOCK_TICKS_PER_SECOND: f64 = 100.0;
const CHECK_EVERY: u64 = 256;

pub fn nanos_per_call(duration: Duration, repeats: usize, mut call: impl FnMut()) -> f64 {
    let mut results: Vec<f64> = (0..repeats.max(1))
        .map(|_| {
            let started = Instant::now();
            let mut calls = 0u64;
            loop {
                for _ in 0..CHECK_EVERY {
                    call();
                }
                calls += CHECK_EVERY;
                let elapsed = started.elapsed();
                if elapsed >= duration {
                    return elapsed.as_secs_f64() * 1e9 / calls as f64;
                }
            }
        })
        .collect();
    results.sort_by(f64::total_cmp);
    results[results.len() / 2]
}

pub fn percentile(sorted: &[u32], quantile: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = (quantile * sorted.len() as f64).ceil() as usize;
    f64::from(sorted[rank.clamp(1, sorted.len()) - 1])
}

pub fn thread_cpu_seconds() -> f64 {
    let Ok(stat) = fs::read_to_string("/proc/thread-self/stat") else {
        return f64::NAN;
    };
    let Some((_, after_name)) = stat.rsplit_once(')') else {
        return f64::NAN;
    };
    let fields: Vec<&str> = after_name.split_whitespace().collect();
    let ticks = |index: usize| fields.get(index).and_then(|field| field.parse::<u64>().ok());
    match (ticks(11), ticks(12)) {
        (Some(user), Some(system)) => (user + system) as f64 / CLOCK_TICKS_PER_SECOND,
        _ => f64::NAN,
    }
}

pub fn thousands(value: f64) -> String {
    let rounded = value.round() as u64;
    let digits = rounded.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

pub fn nanos(value: f64) -> String {
    if value >= 100_000.0 {
        format!("{:.0} µs", value / 1000.0)
    } else if value >= 1_000.0 {
        format!("{:.2} µs", value / 1000.0)
    } else if value >= 100.0 {
        format!("{value:.0} ns")
    } else {
        format!("{value:.1} ns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0.0), "0");
        assert_eq!(thousands(999.4), "999");
        assert_eq!(thousands(1234567.0), "1,234,567");
    }

    #[test]
    fn percentile_picks_the_nearest_rank() {
        let sorted = [1000, 2000, 3000, 4000];
        assert_eq!(percentile(&sorted, 0.5), 2000.0);
        assert_eq!(percentile(&sorted, 0.99), 4000.0);
    }
}
