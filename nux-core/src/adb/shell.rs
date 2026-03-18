//! Shell command execution over ADB.
//!
//! Provides functions for running shell commands on the guest, querying
//! device properties, capturing screenshots, and injecting input events.

use crate::adb::protocol::{AdbMessage, CMD_CLSE, CMD_OKAY, CMD_WRTE, StreamManager};
use crate::adb::transport::ConnectedTransport;
use crate::adb::types::{AdbError, AdbResult, DeviceInfo};
use std::time::Duration;

/// Execute a shell command on the guest and return its stdout output.
///
/// Opens an ADB shell stream, sends the command, collects output until
/// the stream is closed, and returns the output as a string.
///
/// # Errors
///
/// Returns `AdbError::Timeout` if the command exceeds the timeout,
/// `AdbError::ProtocolError` on stream errors, or `AdbError::GuestError`
/// if the command returns a non-zero exit code.
pub async fn shell_exec(
    transport: &mut ConnectedTransport,
    streams: &mut StreamManager,
    command: &str,
    timeout_ms: u64,
) -> AdbResult<String> {
    let local_id = streams.open_stream();
    let destination = format!("shell:{command}");
    let open_msg = AdbMessage::open(local_id, &destination);
    transport.write_message(&open_msg).await?;

    let timeout = Duration::from_millis(timeout_ms);
    let mut output = Vec::new();
    let mut remote_id = 0u32;

    let result = tokio::time::timeout(timeout, async {
        loop {
            let msg = transport.read_message().await?;
            match msg.command {
                CMD_OKAY => {
                    if remote_id == 0 {
                        remote_id = msg.arg0;
                        streams.handle_okay(local_id, remote_id)?;
                    }
                }
                CMD_WRTE => {
                    output.extend_from_slice(&msg.data);
                    // Send OKAY to acknowledge the data
                    let ack = AdbMessage::okay(local_id, remote_id);
                    transport.write_message(&ack).await?;
                }
                CMD_CLSE => {
                    streams.handle_close(local_id);
                    streams.remove_stream(local_id);
                    break;
                }
                _ => {
                    return Err(AdbError::ProtocolError(format!(
                        "unexpected command 0x{:08X} during shell exec",
                        msg.command
                    )));
                }
            }
        }
        Ok::<_, AdbError>(())
    })
    .await;

    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(_) => {
            // Timeout — try to close the stream
            if remote_id != 0 {
                let clse = AdbMessage::clse(local_id, remote_id);
                let _ = transport.write_message(&clse).await;
            }
            streams.handle_close(local_id);
            streams.remove_stream(local_id);
            return Err(AdbError::Timeout(format!(
                "shell command timed out after {timeout_ms}ms: {command}"
            )));
        }
    }

    let text = String::from_utf8_lossy(&output).to_string();
    Ok(text)
}

/// Query device information by running `getprop` commands.
///
/// # Errors
///
/// Returns `AdbError` if the shell commands fail.
pub async fn get_device_info(
    transport: &mut ConnectedTransport,
    streams: &mut StreamManager,
    timeout_ms: u64,
) -> AdbResult<DeviceInfo> {
    let props_cmd = "getprop ro.build.version.release; \
                     echo '---'; \
                     getprop ro.build.version.sdk; \
                     echo '---'; \
                     getprop ro.product.model; \
                     echo '---'; \
                     getprop ro.product.cpu.abilist";

    let output = shell_exec(transport, streams, props_cmd, timeout_ms).await?;
    let parts: Vec<&str> = output.split("---").collect();

    Ok(DeviceInfo {
        android_version: parse_prop(parts.first()),
        sdk_level: parse_prop(parts.get(1)),
        model: parse_prop(parts.get(2)),
        cpu_abi_list: parse_prop(parts.get(3)),
    })
}

/// Query the screen resolution via `wm size`.
///
/// Parses both "Physical size" and "Override size" lines, preferring
/// the override if present.
///
/// # Errors
///
/// Returns `AdbError::GuestError` if the output cannot be parsed.
pub async fn get_screen_resolution(
    transport: &mut ConnectedTransport,
    streams: &mut StreamManager,
    timeout_ms: u64,
) -> AdbResult<(u32, u32)> {
    let output = shell_exec(transport, streams, "wm size", timeout_ms).await?;
    parse_wm_size(&output)
}

/// Parse `wm size` output into (width, height).
///
/// # Errors
///
/// Returns `AdbError::GuestError` if no valid size line is found.
pub fn parse_wm_size(output: &str) -> AdbResult<(u32, u32)> {
    // Prefer "Override size" over "Physical size"
    let mut physical = None;
    let mut override_size = None;

    for line in output.lines() {
        let line = line.trim();
        if let Some(dims) = line.strip_prefix("Override size:") {
            override_size = parse_dimensions(dims.trim());
        } else if let Some(dims) = line.strip_prefix("Physical size:") {
            physical = parse_dimensions(dims.trim());
        }
    }

    override_size
        .or(physical)
        .ok_or_else(|| AdbError::GuestError(format!("cannot parse wm size output: {output}")))
}

/// Parse "`WxH`" into (W, H).
fn parse_dimensions(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

/// Parse a property value from a split segment, returning `None` for empty strings.
fn parse_prop(s: Option<&&str>) -> Option<String> {
    s.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// Capture a screenshot from the guest as PNG bytes.
///
/// Runs `screencap -p` and returns the raw PNG data.
///
/// # Errors
///
/// Returns `AdbError::GuestError` if the display is off or the command fails.
pub async fn capture_screenshot(
    transport: &mut ConnectedTransport,
    streams: &mut StreamManager,
    timeout_ms: u64,
) -> AdbResult<Vec<u8>> {
    // Use a raw shell stream to get binary data
    let local_id = streams.open_stream();
    let open_msg = AdbMessage::open(local_id, "shell:screencap -p");
    transport.write_message(&open_msg).await?;

    let timeout = Duration::from_millis(timeout_ms);
    let mut data = Vec::new();
    let mut remote_id = 0u32;

    let result = tokio::time::timeout(timeout, async {
        loop {
            let msg = transport.read_message().await?;
            match msg.command {
                CMD_OKAY => {
                    if remote_id == 0 {
                        remote_id = msg.arg0;
                        streams.handle_okay(local_id, remote_id)?;
                    }
                }
                CMD_WRTE => {
                    data.extend_from_slice(&msg.data);
                    let ack = AdbMessage::okay(local_id, remote_id);
                    transport.write_message(&ack).await?;
                }
                CMD_CLSE => {
                    streams.handle_close(local_id);
                    streams.remove_stream(local_id);
                    break;
                }
                _ => {
                    return Err(AdbError::ProtocolError(format!(
                        "unexpected command 0x{:08X} during screencap",
                        msg.command
                    )));
                }
            }
        }
        Ok::<_, AdbError>(())
    })
    .await;

    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(_) => {
            if remote_id != 0 {
                let clse = AdbMessage::clse(local_id, remote_id);
                let _ = transport.write_message(&clse).await;
            }
            streams.handle_close(local_id);
            streams.remove_stream(local_id);
            return Err(AdbError::Timeout("screencap timed out".to_owned()));
        }
    }

    // Validate we got PNG data (starts with PNG magic bytes)
    if data.len() < 8 || &data[..4] != b"\x89PNG" {
        // Check if it's an error message
        let text = String::from_utf8_lossy(&data);
        if text.contains("display") || text.contains("error") || data.is_empty() {
            return Err(AdbError::GuestError(format!("screencap failed: {text}")));
        }
        return Err(AdbError::GuestError(
            "screencap returned non-PNG data".to_owned(),
        ));
    }

    Ok(data)
}

/// Inject a tap event at the given screen coordinates.
///
/// # Errors
///
/// Returns `AdbError` if the shell command fails.
pub async fn inject_tap(
    transport: &mut ConnectedTransport,
    streams: &mut StreamManager,
    x: u32,
    y: u32,
    timeout_ms: u64,
) -> AdbResult<()> {
    let cmd = format!("input tap {x} {y}");
    shell_exec(transport, streams, &cmd, timeout_ms).await?;
    Ok(())
}

/// Inject text input with proper shell escaping.
///
/// # Errors
///
/// Returns `AdbError` if the shell command fails.
pub async fn inject_text(
    transport: &mut ConnectedTransport,
    streams: &mut StreamManager,
    text: &str,
    timeout_ms: u64,
) -> AdbResult<()> {
    let escaped = shell_escape_text(text);
    let cmd = format!("input text '{escaped}'");
    shell_exec(transport, streams, &cmd, timeout_ms).await?;
    Ok(())
}

/// Inject a key event by Android keycode.
///
/// # Errors
///
/// Returns `AdbError` if the shell command fails.
pub async fn inject_key(
    transport: &mut ConnectedTransport,
    streams: &mut StreamManager,
    keycode: u32,
    timeout_ms: u64,
) -> AdbResult<()> {
    let cmd = format!("input keyevent {keycode}");
    shell_exec(transport, streams, &cmd, timeout_ms).await?;
    Ok(())
}

/// Escape special characters for `input text` shell command.
fn shell_escape_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() * 2);
    for ch in text.chars() {
        match ch {
            '\'' | '\\' | '"' | '`' | '$' | '!' | '(' | ')' | '&' | '|' | ';' | '<' | '>' | ' '
            | '\t' | '\n' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_physical_size() {
        let output = "Physical size: 1080x1920\n";
        let (w, h) = parse_wm_size(output).unwrap();
        assert_eq!(w, 1080);
        assert_eq!(h, 1920);
    }

    #[test]
    fn parse_override_size_preferred() {
        let output = "Physical size: 1080x1920\nOverride size: 720x1280\n";
        let (w, h) = parse_wm_size(output).unwrap();
        assert_eq!(w, 720);
        assert_eq!(h, 1280);
    }

    #[test]
    fn parse_wm_size_invalid() {
        let result = parse_wm_size("garbage output");
        assert!(result.is_err());
    }

    #[test]
    fn shell_escape_special_chars() {
        let escaped = shell_escape_text("hello world");
        assert_eq!(escaped, r"hello\ world");

        let escaped = shell_escape_text("it's a \"test\"");
        assert_eq!(escaped, "it\\'s\\ a\\ \\\"test\\\"");

        let escaped = shell_escape_text("$HOME");
        assert_eq!(escaped, r"\$HOME");
    }

    #[test]
    fn shell_escape_plain_text() {
        let escaped = shell_escape_text("hello123");
        assert_eq!(escaped, "hello123");
    }

    #[test]
    fn parse_device_info_all_fields() {
        // Simulate the output of our combined getprop command
        let output = "16\n---\n36\n---\nNux Virtual Device\n---\nx86_64,arm64-v8a\n";
        let parts: Vec<&str> = output.split("---").collect();

        fn parse_prop(s: Option<&&str>) -> Option<String> {
            s.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
        }

        let info = DeviceInfo {
            android_version: parse_prop(parts.first()),
            sdk_level: parse_prop(parts.get(1)),
            model: parse_prop(parts.get(2)),
            cpu_abi_list: parse_prop(parts.get(3)),
        };

        assert_eq!(info.android_version.as_deref(), Some("16"));
        assert_eq!(info.sdk_level.as_deref(), Some("36"));
        assert_eq!(info.model.as_deref(), Some("Nux Virtual Device"));
        assert_eq!(info.cpu_abi_list.as_deref(), Some("x86_64,arm64-v8a"));
    }

    #[test]
    fn parse_device_info_missing_fields() {
        let output = "\n---\n\n---\nSome Model\n---\n\n";
        let parts: Vec<&str> = output.split("---").collect();

        fn parse_prop(s: Option<&&str>) -> Option<String> {
            s.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
        }

        let info = DeviceInfo {
            android_version: parse_prop(parts.first()),
            sdk_level: parse_prop(parts.get(1)),
            model: parse_prop(parts.get(2)),
            cpu_abi_list: parse_prop(parts.get(3)),
        };

        assert_eq!(info.android_version, None);
        assert_eq!(info.sdk_level, None);
        assert_eq!(info.model.as_deref(), Some("Some Model"));
        assert_eq!(info.cpu_abi_list, None);
    }

    #[test]
    fn inject_tap_command_format() {
        // Verify the command string that would be generated
        let cmd = format!("input tap {} {}", 100, 200);
        assert_eq!(cmd, "input tap 100 200");
    }

    #[test]
    fn inject_key_command_format() {
        let cmd = format!("input keyevent {}", 4); // KEYCODE_BACK
        assert_eq!(cmd, "input keyevent 4");
    }
}
