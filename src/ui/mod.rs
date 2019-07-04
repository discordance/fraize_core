extern crate iui;
extern crate bus;

use std::thread;
use self::iui::prelude::*;
use self::iui::controls::{Label, Button, VerticalBox, HorizontalBox, Group};
use self::bus::{Bus, BusReader};

use config::{Config};

/// Initialize gui machinery
pub fn initialize_gui(conf: Config) -> UI {

  // Initialize the UI library
  let ui = UI::init().expect("Couldn't initialize UI library");

  // Create a window into which controls can be placed
  let mut win = Window::new(&ui, "Smplr GUI", 960, 480, WindowType::NoMenubar);

  // Create a vertical layout to hold the tracks controls
  let mut vbox = VerticalBox::new(&ui);
  vbox.set_padded(&ui, true);

  // UI for each track
  for (i, t) in conf.tracks.iter().enumerate() {
    // group
    let mut group_hbox = HorizontalBox::new(&ui);
    // fill the hbox
    // look at this trick to transform String to &str from the format macro.
    let mut group = Group::new(&ui, &format!("Track {}", i)[..]);
    group.set_child(&ui, group_hbox);
    vbox.append(&ui, group, LayoutStrategy::Compact);
  }

  // append the main vbox to the window
  win.set_child(&ui, vbox);

  // show the window
  win.show(&ui);

  // kill
  win.on_closing(&ui, |_| {
    ui.quit();
    // brutally quit
    ::std::process::exit(0);
  });

  // return to the main
  return ui
}