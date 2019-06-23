extern crate iui;

use std::thread;
use self::iui::prelude::*;
use self::iui::controls::{Label, Button, VerticalBox, Group};

use config::{Config};

/// Initialize gui machinery
pub fn initialize_gui(conf: Config) {
  // Initialize the UI library
  let ui = UI::init().expect("Couldn't initialize UI library");
  // Create a window into which controls can be placed
  let mut win = Window::new(&ui, "Test App", 200, 200, WindowType::NoMenubar);
  // Show the window
  win.show(&ui);
  // Run the application
  ui.main();
}