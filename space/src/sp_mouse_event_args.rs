//! Port of `SPMouseEventArgs`.

#[derive(Debug, Clone, Copy)]
pub struct SPMouseEventArgs {
    button: i32,
    x: i32,
    y: i32,
    cmd_down: bool,
}

impl SPMouseEventArgs {
    pub fn new(button: i32, x: i32, y: i32) -> Self {
        Self::with_cmd(button, x, y, false)
    }
    pub fn with_cmd(button: i32, x: i32, y: i32, cmd_down: bool) -> Self {
        Self { button, x, y, cmd_down }
    }
    /// 0 = left, 1 = right.
    pub fn get_button(&self) -> i32 {
        self.button
    }
    pub fn get_x(&self) -> i32 {
        self.x
    }
    pub fn get_y(&self) -> i32 {
        self.y
    }
    pub fn get_cmd_down(&self) -> bool {
        self.cmd_down
    }
}
crate::impl_event_args!(SPMouseEventArgs, "SPMouseEventArgs");
