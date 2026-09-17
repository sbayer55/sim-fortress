//! Prompt builders, one module per feature (ai-requirements R9). Each takes
//! plain data extracted from the sim, never `&mut`, and is unit-tested for what
//! it includes rather than by calling a model.

pub mod chronicle;
pub mod designer;
