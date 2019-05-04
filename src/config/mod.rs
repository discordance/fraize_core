extern crate serde;
use serde::{Deserialize};
extern crate toml;


use std::collections::HashMap;
use std::fs::File;

use self::toml::from_str;
use std::io::Read;

#[derive(Debug, Clone, Deserialize)]
/// Config struct
pub struct Config
{
  pub midi_map: MidiMap
}

#[derive(Debug, Clone, Deserialize)]
/// MidiMap struct
pub struct MidiMap
{
  pub cc: HashMap<String, HashMap<String, ::control::ControlMessage>>
}


/// Loads and parse the default config
pub fn load_default() -> Config {
  // @TODO have to be serious here
  let input_path = "./src/config/default.toml";

  // load rson file
  let mut input = String::new();
  let _f = File::open(&input_path).expect("Failed opening file").read_to_string(&mut input);

  let config: Config = match from_str(&input) {
    Ok(x) => x,
    Err(e) => {
      println!("Failed to load config: {}", e);

      ::std::process::exit(1);
    },
  };
  config
}
