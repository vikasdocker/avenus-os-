// Aether Graphics - Output management for the graphics stack
//
// Represents physical scanout targets (connectors): mode sets, DPMS-style
// power state, and backlight control on top of the display topology.

use crate::error::GraphicsError;
use crate::types::DisplayMode;

/// Power state of a physical output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputPowerState {
    On,
    Standby,
    Suspend,
    Off,
}

/// A physical output (connector) with its attached display.
#[derive(Debug, Clone)]
pub struct Output {
    pub name: String,
    pub connected: bool,
    pub modes: Vec<DisplayMode>,
    pub active_mode: Option<DisplayMode>,
    pub power: OutputPowerState,
    /// Backlight level in percent (0-100), when supported.
    pub backlight_percent: u8,
}

/// Manages all physical outputs of the system.
pub struct OutputManager {
    outputs: Vec<Output>,
}

impl OutputManager {
    pub fn new() -> Self {
        Self {
            outputs: Vec::new(),
        }
    }

    /// Adds an output connector with its supported mode list.
    pub fn add_output(&mut self, name: &str, modes: Vec<DisplayMode>) -> Result<usize, GraphicsError> {
        if name.is_empty() {
            return Err(GraphicsError::Output(
                "Output name must not be empty".to_string(),
            ));
        }
        if self.outputs.iter().any(|o| o.name == name) {
            return Err(GraphicsError::Output(format!(
                "Output '{}' already registered",
                name
            )));
        }
        self.outputs.push(Output {
            name: name.to_string(),
            connected: true,
            modes,
            active_mode: None,
            power: OutputPowerState::On,
            backlight_percent: 100,
        });
        Ok(self.outputs.len() - 1)
    }

    /// Activates a mode on an output by index into its mode list.
    pub fn set_mode(&mut self, name: &str, mode_index: usize) -> Result<(), GraphicsError> {
        let out = self
            .outputs
            .iter_mut()
            .find(|o| o.name == name)
            .ok_or_else(|| GraphicsError::Output(format!("Output '{}' not found", name)))?;
        if !out.connected {
            return Err(GraphicsError::Output(format!(
                "Output '{}' is disconnected",
                name
            )));
        }
        if mode_index >= out.modes.len() {
            return Err(GraphicsError::Mode(format!(
                "Mode index {} not supported by '{}'",
                mode_index, name
            )));
        }
        out.active_mode = Some(out.modes[mode_index]);
        Ok(())
    }

    /// Sets the power state of an output. Powering off clears the active mode.
    pub fn set_power(&mut self, name: &str, power: OutputPowerState) -> Result<(), GraphicsError> {
        let out = self
            .outputs
            .iter_mut()
            .find(|o| o.name == name)
            .ok_or_else(|| GraphicsError::Output(format!("Output '{}' not found", name)))?;
        out.power = power;
        if power == OutputPowerState::Off {
            out.active_mode = None;
        }
        Ok(())
    }

    /// Sets backlight brightness (0-100).
    pub fn set_backlight(&mut self, name: &str, percent: u8) -> Result<(), GraphicsError> {
        if percent > 100 {
            return Err(GraphicsError::InvalidParameter(
                "Backlight percent must be within 0..=100".to_string(),
            ));
        }
        let out = self
            .outputs
            .iter_mut()
            .find(|o| o.name == name)
            .ok_or_else(|| GraphicsError::Output(format!("Output '{}' not found", name)))?;
        out.backlight_percent = percent;
        Ok(())
    }

    /// Marks a connector as physically (dis)connected; disconnection clears mode.
    pub fn set_connected(&mut self, name: &str, connected: bool) -> Result<(), GraphicsError> {
        let out = self
            .outputs
            .iter_mut()
            .find(|o| o.name == name)
            .ok_or_else(|| GraphicsError::Output(format!("Output '{}' not found", name)))?;
        out.connected = connected;
        if !connected {
            out.active_mode = None;
        }
        Ok(())
    }

    /// Returns all managed outputs.
    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    /// Returns names of outputs that are connected, powered on, and have an active mode.
    pub fn scanning_outputs(&self) -> Vec<String> {
        self.outputs
            .iter()
            .filter(|o| {
                o.connected && o.power == OutputPowerState::On && o.active_mode.is_some()
            })
            .map(|o| o.name.clone())
            .collect()
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(w: u32, h: u32) -> DisplayMode {
        DisplayMode {
            width: w,
            height: h,
            refresh_rate: 60,
        }
    }

    #[test]
    fn mode_set_and_scanout() {
        let mut om = OutputManager::new();
        om.add_output("HDMI-A-1", vec![mode(1920, 1080)])
            .unwrap_or_else(|e| panic!("{e}"));
        om.set_mode("HDMI-A-1", 0).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(om.scanning_outputs(), vec!["HDMI-A-1".to_string()]);
    }

    #[test]
    fn power_off_clears_mode() {
        let mut om = OutputManager::new();
        om.add_output("eDP-1", vec![mode(1280, 800)])
            .unwrap_or_else(|e| panic!("{e}"));
        om.set_mode("eDP-1", 0).unwrap_or_else(|e| panic!("{e}"));
        om.set_power("eDP-1", OutputPowerState::Off)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(om.scanning_outputs().is_empty());
    }

    #[test]
    fn duplicate_output_rejected() {
        let mut om = OutputManager::new();
        om.add_output("DP-1", vec![]).unwrap_or_else(|e| panic!("{e}"));
        assert!(om.add_output("DP-1", vec![]).is_err());
    }

    #[test]
    fn backlight_range_checked() {
        let mut om = OutputManager::new();
        om.add_output("LVDS-1", vec![]).unwrap_or_else(|e| panic!("{e}"));
        assert!(om.set_backlight("LVDS-1", 101).is_err());
        om.set_backlight("LVDS-1", 50).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(om.outputs()[0].backlight_percent, 50);
    }
}
