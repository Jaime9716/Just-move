//! Reloj monotónico único del proceso (contracts/clock.h). Arquitecto.
use std::sync::OnceLock;
use std::time::Instant;

static BASE: OnceLock<Instant> = OnceLock::new();

/// Nanosegundos monotónicos desde una base fija del proceso (siempre > 0).
pub fn now_ns() -> i64 {
    let base = *BASE.get_or_init(|| Instant::now() - std::time::Duration::from_secs(1));
    base.elapsed().as_nanos() as i64
}

#[cfg(test)]
mod tests {
    #[test]
    fn monotonic() {
        let a = super::now_ns();
        let b = super::now_ns();
        assert!(a > 0 && b >= a);
    }
}
