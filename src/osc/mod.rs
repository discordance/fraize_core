extern crate bus;
extern crate rosc;
extern crate serde;
extern crate serde_json;

use serde::{Serialize};
use self::bus::{BusReader};
use self::rosc::{OscPacket, OscType, OscMessage};
use self::rosc::encoder;
use std::thread;
use config::{Config};
use control::{ControlMessage};
use std::net::{UdpSocket, SocketAddr, SocketAddrV4};
use std::str::FromStr;
use self::serde_json::to_string;

/// OSCRemoteControl keeps track of the remote controller app that control this smplr instance
struct OSCRemoteControl {
  address: Option<SocketAddr>
}

/// Port of the remote OSC app
const OSC_REMOTE_CONTROL_PORT: u16 = 6666;

/// Initialize the OSC thread / routines
pub fn initialize_osc(conf: Config) -> (thread::JoinHandle<()>, BusReader<ControlMessage>) {
  // init the control bus
  let mut control_bus = ::control::initialize_control();

  // bus channel to communicate from the midi callback to audio tracks
  let outer_rx = control_bus.add_rx();

  // init the osc thread
  let osc_thread = thread::spawn(move || {

    // keep track of the remote UI controller using this datastruct
    let mut osc_controller = OSCRemoteControl {
      address: None,
    };

    // init host address
    let host_addr = SocketAddrV4::from_str("0.0.0.0:6667").unwrap();

    // init the receiving socket
    let socket = UdpSocket::bind(host_addr).unwrap();
    println!("osc: Listening to {}", host_addr);

    // OSC buffer
    let mut buf = [0u8; rosc::decoder::MTU];

    // OSC loop
    loop {
      match socket.recv_from(&mut buf) {
        Ok((size, addr)) => {
          // println!("osc: Received packet with size {} from: {}", size, addr);
          let packet = rosc::decoder::decode(&buf[..size]).unwrap();
          handle_incoming_packet(packet, addr, &mut osc_controller, &socket, &conf);
        }
        Err(e) => {
          println!("osc: Error receiving from socket: {}", e);
          break;
        }
      }
    }
  });

  // return thread handle and receiver
  return (osc_thread, outer_rx)
}

// handle an incoming os packet
fn handle_incoming_packet(packet: OscPacket,
                          from: SocketAddr,
                          osc_controller: &mut OSCRemoteControl,
                          socket: &UdpSocket,
                          conf: &Config
    ) {
    match packet {
        OscPacket::Message(msg) => {
          // route this packet
          match msg.addr.as_str() {
            // ping is important to keep the state of connection
            "/smplr/ping" => {
              handle_ping(from, osc_controller, socket, msg)
            },
            // remote control is asking for config toml as serialized string
            "/smplr/get_config" => {
              // serialize the conf to hson string
              // can't use toml because datastruct support is too limited
              let serialized_conf = to_string(conf).unwrap();

              // creates set_config osc message
              let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
                addr: "/smplr/set_config".to_string(),
                args: Some(vec![OscType::String(serialized_conf)]),
              })).unwrap();

              // extract addr
              let send_to = osc_controller.address.unwrap();

              // send back the config
              socket.send_to(&msg_buf, send_to).unwrap();
            }
            _ => {
              println!("osc: unimplemented adress: {:?}", msg.addr);
            }
          };
        }
        OscPacket::Bundle(bundle) => {
            // println!("osc: OSC Bundle: {:?}", bundle);
        }
    }
}

// handle ping form controller
fn handle_ping(from: SocketAddr, osc_controller: &mut OSCRemoteControl, socket: &UdpSocket, msg: OscMessage) {
  match msg.args {
    Some(args) => {
      let rnd_ping = &args[0];
      match rnd_ping {
        OscType::Int(r) => {
          // init the remote control
          // change port to expected remote port
          let mut new_from = from.clone();
          new_from.set_port(OSC_REMOTE_CONTROL_PORT);
          if osc_controller.address == None {
            osc_controller.address = Some(new_from);
          }

          // creates pingback osc message
          let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
            addr: "/smplr/ping_back".to_string(),
            args: Some(vec![OscType::Int(*r)]),
          })).unwrap();

          // extract addr
          let send_to = osc_controller.address.unwrap();

          // send back
          socket.send_to(&msg_buf, send_to).unwrap();
        },
        _ => {
          println!("osc: incorrect type ping, ignoring ...")
        }
      }
    }
    None => println!("osc: No arguments in ping, ignoring ..."),
  }
}
