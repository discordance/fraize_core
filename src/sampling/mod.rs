use sample_gen::SmartBuffer;
use std::error::Error;
use std::fs;

use config::Config;

/// SampleLib Manage samples loading and analytics.
/// Its like a In-Memory Sample Database
pub struct SampleLib {
    /// In-Memory SmartBuffer Store.
    /// SampleLib is organized in banks.
    buffers: Vec<Vec<SmartBuffer>>,
    /// empty buff for ref
    empty_buff: SmartBuffer,
}

impl SampleLib {
    /// Gets the first sample of the bank, returns Empty if not found
    pub fn get_first_sample_name(&self, bank: usize) -> &str {
        match self.buffers.get(bank) {
            Some(b) => {
                // take the first
                let first = match b.first() {
                    Some(x) => {
                        return &x.file_name;
                    }
                    None => return "",
                };
            }
            None => return "",
        };
    }

    /// Gets the first sample of the bank, returns Empty if not found
    pub fn get_first_sample(&self, bank: usize) -> &SmartBuffer {
        match self.buffers.get(bank) {
            Some(b) => {
                // take the first
                let first = match b.first() {
                    Some(x) => {
                        return x;
                    }
                    None => return &self.empty_buff,
                };
            }
            None => return &self.empty_buff,
        };
    }

    /// Gets the sample of the bank by double index position, returns Empty if not found
    pub fn get_sample_by_pos(&self, pos: (usize, usize)) -> &SmartBuffer {
        match self.buffers.get(pos.0) {
            Some(b) => {
                // take the pos
                let first = match b.get(pos.1) {
                    Some(x) => {
                        return x;
                    }
                    None => return &self.empty_buff,
                };
            }
            None => return &self.empty_buff,
        };
    }

    /// Gets the sample of the bank by name
    pub fn get_sample_by_name(&self, bank: usize, name: &str) -> &SmartBuffer {
        match self.buffers.get(bank) {
            Some(b) => {
                // take the matching string
                let found = b.iter().find(|&sb| {
                    return sb.file_name == name;
                });

                // match
                match found {
                    None => {
                        return &self.empty_buff;
                    }
                    Some(sb) => return sb,
                }
            }
            None => return &self.empty_buff,
        };
    }

    /// Gets the next sample given a name and a bank, wrapping around
    pub fn get_sibling_sample(&self, bank: usize, name: &str, order: isize) -> &SmartBuffer {
        match self.buffers.get(bank) {
            Some(b) => {
                // take the matching string
                let found = b.iter().position(|sb| {
                    return sb.file_name == name;
                });

                // match
                match found {
                    None => {
                        return &self.empty_buff;
                    }
                    Some(pos) => {
                        let new_pos = pos as isize + order;
                        let new_pos = (new_pos + (b.len() as isize)) as usize % b.len();
                        return b.get(new_pos).unwrap();
                    }
                }
            }
            None => return &self.empty_buff,
        };
    }
}

/// init the SampleLib, loads the samples
pub fn init_lib(conf: Config) -> Result<SampleLib, Box<dyn Error>> {
    // init lib
    let mut lib = SampleLib {
        buffers: Vec::new(),
        empty_buff: SmartBuffer::new_empty(),
    };

    // directory walk
    let paths = fs::read_dir(conf.audio_root)?;

    for bank_path in paths {
        // somewhat ugly
        let b = bank_path?;
        let bank_name = b.file_name();
        let ftype = b.file_type()?;

        match bank_name.to_str().unwrap() {
            // our junk filter
            ".DS_Store" => {}
            _ => {
                // is it a directory ?
                if ftype.is_dir() {
                    let mut buffs = Vec::<SmartBuffer>::new();
                    // read the samples
                    let audio_paths = fs::read_dir(b.path())?;

                    for file_path in audio_paths {
                        let f = file_path?;
                        let file_name = f.file_name();
                        let ftype = f.file_type()?;

                        // filter out crap
                        match file_name.to_str().unwrap() {
                            // our junk filter
                            ".DS_Store" => {}
                            _ => {
                                // load file as smart buffer
                                if !ftype.is_dir() {
                                    // load smart buffer
                                    let mut buffer = SmartBuffer::new_empty();
                                    let fpath = f.path();
                                    let fpath = fpath.to_str().unwrap(); // NoneError doesnt not implem Boxed Error
                                                                         // sets name
                                    buffer.file_name = String::from(file_name.to_str().unwrap());
                                    buffer.load_wave(fpath);

                                    // push
                                    buffs.push(buffer);
                                }
                            }
                        }
                    }
                    // finally push in lib
                    lib.buffers.push(buffs);
                }
            }
        }
        //
    }

    // yeah
    Ok(lib)
}
