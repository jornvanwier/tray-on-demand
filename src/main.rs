#[macro_use]
extern crate failure;
extern crate x11;
extern crate xcb;
extern crate zmq;

mod error;
mod utils;
mod tray_client;
mod tray_manager;

use error::TrayError;
use tray_manager::TrayManager;
use std::thread;
use std::sync::Arc;
use std::time;

fn main() {
    let mut manager = TrayManager::init().expect("Couldn't create tray");
    println!("{}", manager);

    let conn = manager.connection.clone();
    let handle = manager.tray_window_handle;

    thread::spawn(move || run_zmq_server(&conn, handle));

    manager.run_loop();
}

fn run_zmq_server(
    connection: &xcb::Connection,
    tray_window_handle: xcb::Window,
) -> Result<(), TrayError> {
    let context = zmq::Context::new();
    let responder = context.socket(zmq::REP).map_err(
        |e| TrayError::ZMQ { from: e },
    )?;

    assert!(responder.bind("tcp://*:5555").is_ok());

    let mut msg = zmq::Message::new().map_err(|e| TrayError::ZMQ { from: e })?;
    loop {
        responder.recv(&mut msg, 0).map_err(
            |e| TrayError::ZMQ { from: e },
        )?;
        if let Some(msg) = msg.as_str() {
            println!("Received {}", msg);
            responder.send_str("ok", 0).map_err(
                |e| TrayError::ZMQ { from: e },
            )?;

            match msg {
                "show" => {
                    xcb::map_window_checked(&connection, tray_window_handle)
                        .request_check()
                        .map_err(|e| TrayError::GenericXcb { from: e })?
                }
                "hide" => {
                    xcb::unmap_window_checked(&connection, tray_window_handle)
                        .request_check()
                        .map_err(|e| TrayError::GenericXcb { from: e })?
                }
                "toggle" => {
                    let map_state = xcb::get_window_attributes(&connection, tray_window_handle)
                        .get_reply()
                        .map_err(|e| TrayError::GenericXcb { from: e })?
                        .map_state();
                    println!("Map state: {}", map_state);
                    match u32::from(map_state) {
                        xcb::MAP_STATE_VIEWABLE => {
                            xcb::unmap_window_checked(&connection, tray_window_handle)
                                .request_check()
                                .map_err(|e| TrayError::GenericXcb { from: e })?;
                        }
                        xcb::MAP_STATE_UNVIEWABLE => {
                            // Window is on another workspace. Bring it into view by remapping it
                            xcb::unmap_window_checked(&connection, tray_window_handle)
                                .request_check()
                                .map_err(|e| TrayError::GenericXcb { from: e })?;
                            xcb::map_window_checked(&connection, tray_window_handle)
                                .request_check()
                                .map_err(|e| TrayError::GenericXcb { from: e })?;
                        }
                        xcb::MAP_STATE_UNMAPPED => {
                            xcb::map_window_checked(&connection, tray_window_handle)
                                .request_check()
                                .map_err(|e| TrayError::GenericXcb { from: e })?;
                        }
                        _ => {}
                    }
                }
                "quit" => {
                    xcb::destroy_window_checked(&connection, tray_window_handle)
                        .request_check()
                        .map_err(|e| TrayError::GenericXcb { from: e })?
                }
                _ => {}
            }
        } else {
            println!("{}", TrayError::new("ZMQ message is invalid UTF-8"));
            responder.send_str("err", 0).map_err(
                |e| TrayError::ZMQ { from: e },
            )?;
        }
    }
}
