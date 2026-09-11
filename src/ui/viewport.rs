//! Viewport math and shared layout constants for the live screens.

/// Clamp the scroll origin so a `viewport_w × viewport_h` window never slides
/// past the edge of a `world_w × world_h` grid. Defined once and reused by every
/// centring rule in later chunks. World size and viewport size are independent.
pub fn max_origin(world_w: usize, world_h: usize, viewport_w: usize, viewport_h: usize) -> (usize, usize) {
    (world_w.saturating_sub(viewport_w), world_h.saturating_sub(viewport_h))
}

/// Sidebar width when expanded.
pub const SIDEBAR_W: u16 = 43;
/// Collapsed sidebar gutter width.
pub const GUTTER_W: u16 = 3;
/// Map panel is collapsed away below this width.
pub const MIN_MAP_W: u16 = 24;
/// Rows reserved below the map panel: ticker + spacer + status bar.
pub const MAP_CHROME_ROWS: u16 = 3;
/// Rows reserved below the map panel when the terminal is short (ticker + status).
pub const MAP_CHROME_ROWS_TIGHT: u16 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_max_origin_small_world() {
        // World smaller than the viewport → no scrolling.
        assert_eq!(max_origin(100, 30, 200, 60), (0, 0));
        // World larger than the viewport → clamp to the delta.
        assert_eq!(max_origin(150, 40, 110, 40), (40, 0));
        assert_eq!(max_origin(200, 60, 110, 40), (90, 20));
        // Equal size → origin 0.
        assert_eq!(max_origin(110, 40, 110, 40), (0, 0));
    }
}
