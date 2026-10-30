//! Port of `SPVisualization`: a pluggable audio visualization with tweakable
//! parameters.

use crate::sp_color::SPColor;

#[derive(Debug, Clone, PartialEq)]
pub enum SPVisualizationParamData {
    Float { value: f32, min: f32, max: f32, step: f32 },
    Bool { value: bool },
    Color { value: SPColor },
    Enum { value: i32, options: Vec<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SPVisualizationParam {
    /// Unique persistence key.
    pub name: String,
    /// Human-readable label shown in the parameter panel.
    pub label: String,
    pub data: SPVisualizationParamData,
}

impl SPVisualizationParam {
    pub fn make_float(name: &str, label: &str, value: f32, min: f32, max: f32, step: f32) -> Self {
        Self::make(name, label, SPVisualizationParamData::Float { value, min, max, step })
    }
    pub fn make_bool(name: &str, label: &str, value: bool) -> Self {
        Self::make(name, label, SPVisualizationParamData::Bool { value })
    }
    pub fn make_color(name: &str, label: &str, value: SPColor) -> Self {
        Self::make(name, label, SPVisualizationParamData::Color { value })
    }
    pub fn make_enum(name: &str, label: &str, value: i32, options: Vec<String>) -> Self {
        Self::make(name, label, SPVisualizationParamData::Enum { value, options })
    }
    fn make(name: &str, label: &str, data: SPVisualizationParamData) -> Self {
        Self { name: name.to_string(), label: label.to_string(), data }
    }
}

pub trait SPVisualization {
    fn params(&mut self) -> &mut Vec<SPVisualizationParam>;
    fn get_name(&self) -> String;
    fn render_frame(&mut self, pcm: &[f32], fft: &[f32], width: i32, height: i32);
}
