//! Project model and music logic for Orchestre. No audio or UI code here.

pub mod edit;
pub mod file;
pub mod instrument;
pub mod project;
pub mod theory;
pub mod time;

pub use instrument::*;
pub use project::*;
pub use theory::*;
pub use time::*;
