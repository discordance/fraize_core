extern crate serde;
use serde::Deserialize;
extern crate dirs;
extern crate toml;

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;

use self::toml::from_str;
use std::io::Read;

#[derive(Debug, Clone, Deserialize, Serialize)]
/// Config struct
pub struct Config {
    pub tracks: Vec<TrackType>,
    pub audio_root: String,
    pub midi_map: MidiMap,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
/// TrackType enum
pub enum TrackType {
    SlicerGen { bank: usize },
    RePitchGen { bank: usize },
    PVOCGen { bank: usize },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
/// MidiMap struct
pub struct MidiMap {
    pub cc: HashMap<String, HashMap<String, ::control::ControlMessage>>,
}

/// Loads and parse the default config
pub fn load_default() -> Config {
    let home_dir = dirs::home_dir().unwrap();
    let input_path = home_dir.join("smplr/config.toml");

    println!("{}", input_path.to_str().unwrap());

    let config: Config = match load_conf(input_path.to_str().unwrap()) {
        Ok(x) => x,
        Err(e) => {
            println!("Failed to load config: {}", e);

            ::std::process::exit(1);
        }
    };
    config
}

/// Loads and parse config
fn load_conf(path: &str) -> Result<Config, Box<dyn Error>> {
    // load toml file
    let mut input = String::new();

    // open
    let mut f = File::open(&path)?;
    f.read_to_string(&mut input).unwrap();

    // parse
    let conf = from_str(&input)?;

    // ret
    Ok(conf)
}
