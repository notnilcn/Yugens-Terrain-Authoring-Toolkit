pub fn growth_trauma(elapsed: f64, growth_time: f64) -> f64 {
    if growth_time <= 0.0 {
        1.0
    } else {
        (elapsed / growth_time).min(1.0)
    }
}

pub fn decay_trauma(countdown: f64, decay_time: f64) -> f64 {
    if decay_time <= 0.0 {
        0.0
    } else {
        (1.0 - (countdown / decay_time)).max(0.0)
    }
}
