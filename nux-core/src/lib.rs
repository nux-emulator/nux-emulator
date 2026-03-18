//! nux-core — Core library for the Nux Android emulator.
//!
//! This crate provides VM management, display pipeline, input routing,
//! keymap engine, ADB bridge, root manager, config, networking, and audio
//! functionality. It has no UI dependencies.

pub mod adb;
pub mod audio;
pub mod config;
pub mod display;
pub mod gservices;
pub mod input;
pub mod keymap;
pub mod network;
pub mod root;
pub mod vm;
