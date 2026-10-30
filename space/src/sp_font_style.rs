//! Port of `SPFontStyle`.

#[derive(Debug, Clone, PartialEq)]
pub struct SPFontStyle {
    pub height: i32,
    pub weight: i32,
    pub italic: bool,
    pub underline: bool,
    pub family: String,
}

impl SPFontStyle {
    pub fn new(family: &str, height: i32, weight: i32, italic: bool, underline: bool) -> Self {
        Self {
            height,
            weight,
            italic,
            underline,
            family: family.to_string(),
        }
    }
    pub fn get_height(&self) -> i32 {
        self.height
    }
    pub fn get_weight(&self) -> i32 {
        self.weight
    }
    pub fn is_italic(&self) -> bool {
        self.italic
    }
    pub fn is_underline(&self) -> bool {
        self.underline
    }
    pub fn get_family(&self) -> String {
        self.family.clone()
    }
}

impl Default for SPFontStyle {
    fn default() -> Self {
        Self::new("Tahoma", 11, 1, false, false)
    }
}
