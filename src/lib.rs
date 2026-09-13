//! Sim Fortress: a terminal predator/prey/evolution/resource simulation.
//!
//! The crate is split into a pure, deterministic `sim` core (no terminal code)
//! and a `ui` layer that renders it with `ratatui`.

// `needless_range_loop`/`too_many_arguments`/`unnecessary_sort_by`: the sim's
// hot loops read more clearly with explicit indices and wide signatures.
//
// `indexing_slicing`: every index in the crate is into a fixed-size array
// bounded by the six `SpeciesId`s, eight regions or the trait count, or into a
// `Vec`/slice whose index comes from a checked `0..len()` loop. Rewriting those
// ~500 sites as `get`/`get_mut` would obscure the simulation's arithmetic
// without preventing any reachable panic.
//
// `many_single_char_names`: the simulation and chart code is numeric, where
// `x`/`y`/`i`/`j`/`w`/`h`/`t` are the conventional names; renaming them to
// satisfy the (deliberately strict) 4-name threshold would hurt readability.
//
// The floating-point lints (`suboptimal_flops`, `imprecise_flops`,
// `manual_midpoint`) are intentionally not satisfied: their suggestions rewrite
// arithmetic to `mul_add`/`midpoint`, whose different rounding changes
// simulation results. `sim::tests::checksum_is_fnv_stable` pins the exact
// checksum so such algorithm changes fail loudly; keeping `a * b + c` as
// written is what preserves determinism and save compatibility.
#![allow(
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::unnecessary_sort_by,
    clippy::indexing_slicing,
    clippy::many_single_char_names,
    clippy::suboptimal_flops,
    clippy::imprecise_flops,
    clippy::manual_midpoint
)]

/// Explicit numeric cast used throughout the simulation.
///
/// The clippy cast lints (`as_conversions`, `cast_possible_truncation`,
/// `cast_precision_loss`, `cast_sign_loss`, `cast_possible_wrap`) are enabled
/// project-wide, so the `as` operator is confined to this one reviewed macro.
/// It preserves the exact `as` semantics (wrapping, truncation, rounding) and
/// works in `const` contexts, unlike `From`/`TryFrom`.
#[macro_export]
macro_rules! cast {
    ($e:expr => $t:ty) => {{
        #[allow(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss,
            clippy::cast_possible_wrap
        )]
        let v = $e as $t;
        v
    }};
}

pub mod glyphs;
pub mod sim;
pub mod theme;
pub mod ui;
pub mod widgets;
