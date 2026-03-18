//! Native scrcpy client — receives H.264 frames and decodes them.
//!
//! Implements the scrcpy protocol:
//! 1. Push scrcpy-server to device via ADB
//! 2. Start server via ADB shell
//! 3. Connect to video stream socket
//! 4. Decode H.264 frames with FFmpeg
//! 5. Send decoded RGB frames to the GTK4 renderer

pub mod connection;
pub mod decoder;
pub mod server;
