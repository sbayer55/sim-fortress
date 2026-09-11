//! Single source of truth for the two map distance metrics (FR Scope).
//!
//! Cells are twice as tall as they are wide on screen, so `dist` halves the x
//! difference (ellipse metric). `cheb` is plain Chebyshev, used for adjacency.

/// Ellipse metric with a 2:1 cell aspect: `sqrt((dx/2)² + dy²)`.
pub fn dist(x0: usize, y0: usize, x1: usize, y1: usize) -> f32 {
    let dx = (x0 as f32 - x1 as f32) / 2.0;
    let dy = y0 as f32 - y1 as f32;
    (dx * dx + dy * dy).sqrt()
}

/// Chebyshev distance `max(|dx|, |dy|)`, used for 8-neighbour adjacency.
pub fn cheb(x0: usize, y0: usize, x1: usize, y1: usize) -> usize {
    let dx = (x0 as i64 - x1 as i64).unsigned_abs() as usize;
    let dy = (y0 as i64 - y1 as i64).unsigned_abs() as usize;
    dx.max(dy)
}

#[cfg(test)]
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
