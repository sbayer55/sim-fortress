//! The `Component` trait every widget implements, and the constraints the
//! stacks and column blocks share. See `docs/components/README.md`.
//!
//! A component is a plain struct built with builder methods. It reports its
//! size before it is drawn (`height` for a width, `min_width`) so containers
//! can lay children out, and it renders into a [`Buffer`], never a `Frame`, so
//! the same code works on screen and on an off-screen scroll canvas.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Implemented by every component in `src/widgets`.
pub trait Component {
    /// Rows needed at `width`. Row components return 1.
    fn height(&self, width: u16) -> u16;
    /// Narrowest width that still shows every required slot.
    fn min_width(&self) -> u16;
    /// Draw into `area`. Never writes outside it.
    fn render(&self, buf: &mut Buffer, area: Rect);
}

impl<T: Component + ?Sized> Component for &T {
    fn height(&self, width: u16) -> u16 {
        (**self).height(width)
    }
    fn min_width(&self) -> u16 {
        (**self).min_width()
    }
    fn render(&self, buf: &mut Buffer, area: Rect) {
        (**self).render(buf, area);
    }
}

impl<T: Component + ?Sized> Component for Box<T> {
    fn height(&self, width: u16) -> u16 {
        (**self).height(width)
    }
    fn min_width(&self) -> u16 {
        (**self).min_width()
    }
    fn render(&self, buf: &mut Buffer, area: Rect) {
        (**self).render(buf, area);
    }
}

/// Owned children, for section builders that *return* their rows instead of
/// drawing them. Feed them to a stack with [`super::VStack::from_boxes`].
pub type Rows<'a> = Vec<Box<dyn Component + 'a>>;

/// How much of one axis a child of a stack or a column of a block takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint {
    /// Ask the child: `height(width)` in a `VStack`, `min_width()` in an `HStack`.
    Auto,
    /// Exactly `n` rows or columns.
    Fixed(u16),
    /// A share of what is left after `Auto`, `Fixed` and `Min` children, by
    /// weight; the last `Fill` child takes the rounding remainder.
    Fill(u16),
    /// Columns only: measured over every row of the block, never below `n`.
    Min(u16),
}

/// Where a cell's text sits inside the width it is given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Right,
    Center,
}

/// Resolve one axis: one size per constraint.
///
/// `natural[i]` is the child's own size, used by `Auto` and as the measured
/// width of a `Min` column. `Fill` children share `total` minus the fixed part
/// minus `gap × (n − 1)`, weight-proportional, remainder to the last `Fill`.
pub(crate) fn distribute(kinds: &[Constraint], natural: &[u16], total: u16, gap: u16) -> Vec<u16> {
    let gaps = gap.saturating_mul(crate::cast!(kinds.len().saturating_sub(1) => u16));
    let mut sizes: Vec<u16> = kinds
        .iter()
        .zip(natural.iter().chain(std::iter::repeat(&0)))
        .map(|(k, &n)| match *k {
            Constraint::Auto => n,
            Constraint::Fixed(w) => w,
            Constraint::Min(floor) => n.max(floor),
            Constraint::Fill(_) => 0,
        })
        .collect();
    let used = sizes.iter().map(|&s| u32::from(s)).sum::<u32>() + u32::from(gaps);
    let left = u32::from(total).saturating_sub(used);
    let weights: u32 = kinds.iter().map(|k| if let Constraint::Fill(w) = k { u32::from(*w) } else { 0 }).sum();
    let last_fill = kinds.iter().rposition(|k| matches!(k, Constraint::Fill(_)));
    let mut given = 0u32;
    for (i, k) in kinds.iter().enumerate() {
        let Constraint::Fill(w) = *k else { continue };
        let share = if Some(i) == last_fill {
            left.saturating_sub(given)
        } else if weights == 0 {
            0
        } else {
            (left * u32::from(w)).div_euclid(weights)
        };
        given += share;
        if let Some(slot) = sizes.get_mut(i) {
            *slot = crate::cast!(share => u16);
        }
    }
    sizes
}
