//! Shared drawing code: the first-class components (`docs/components`) and the
//! free-function helpers the screens still call, which now wrap them.

pub mod bars;
pub mod component;
pub mod divider;
pub mod key_hint;
pub mod map;
pub mod panel;
pub mod scroll;
pub mod stack;
pub mod status;
pub mod text;
pub mod ticker;
pub mod trend;
pub mod util;

pub use component::{Align, Component, Constraint, Rows};
pub use divider::Divider;
pub use key_hint::{KeyHint, KeyHintRow};
pub use panel::{Kind, Panel};
pub use stack::{HStack, Spacer, VStack};
pub use status::StatusBar;
pub use bars::{Bar, Inverted, LabeledBar, RangeBar, Sparkline};
pub use text::Text;
pub use ticker::Ticker;
pub use trend::TrendArrow;
