//! lyrichub-align (PLAN-plugins.md, P3): real word times from the player's sound. The parts live
//! in this library, so the P0 check (examples/jamendo.rs) runs the same code as the plugin.

pub mod align;
pub mod capture;
pub mod ctc;
pub mod features;
pub mod fft;
pub mod model;
pub mod recorder;
pub mod resample;
pub mod rpc;
pub mod status;
pub mod text;
pub mod worker;

#[cfg(test)]
mod testdata;
