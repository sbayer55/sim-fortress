//! Single source of truth for the two map distance metrics (FR Scope).
//!
//! Cells are twice as tall as they are wide on screen, so `dist` halves the x
//! difference (ellipse metric). `cheb` is plain Chebyshev, used for adjacency.

/// Ellipse metric with a 2:1 cell aspect: `sqrt((dx/2)² + dy²)`.
pub fn dist(x0: usize, y0: usize, x1: usize, y1: usize) -> f32 {
    let dx = (crate::cast!(x0 => f32) - crate::cast!(x1 => f32)) / 2.0;
    let dy = crate::cast!(y0 => f32) - crate::cast!(y1 => f32);
    (dx * dx + dy * dy).sqrt()
}

/// Chebyshev distance `max(|dx|, |dy|)`, used for 8-neighbour adjacency.
pub fn cheb(x0: usize, y0: usize, x1: usize, y1: usize) -> usize {
    let dx = crate::cast!((crate::cast!(x0 => i64) - crate::cast!(x1 => i64)).unsigned_abs() => usize);
    let dy = crate::cast!((crate::cast!(y0 => i64) - crate::cast!(y1 => i64)).unsigned_abs() => usize);
    dx.max(dy)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {

    use super::*;

    #[test]
    fn dist_halves_x() {
        assert_eq!(dist(0, 0, 4, 0), 2.0); // dx 4 → /2 = 2
        assert_eq!(dist(0, 0, 0, 3), 3.0);
        assert_eq!(dist(0, 0, 4, 3), (4.0f32 + 9.0).sqrt()); // dx 2, dy 3
    }

    #[test]
    fn cheb_is_max_component() {
        assert_eq!(cheb(0, 0, 1, 1), 1);
        assert_eq!(cheb(0, 0, 3, 1), 3);
        assert_eq!(cheb(5, 5, 5, 5), 0);
        assert_eq!(cheb(0, 0, 1, 0), 1);
    }
}
