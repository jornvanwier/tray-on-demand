use std;
use xcb;

use utils;

use std::sync::Arc;
use std::fmt::{self, Display};
use error::TrayError;
use tray_client::TrayClient;

const ICON_SIZE: u32 = 32;
const CLIENT_Y: u32 = 10;
const TRAY_LEFT_PAD: u32 = 10;
const SYSTEM_TRAY_REQUEST_DOCK: u32 = 0;

pub struct TrayManager {
    pub tray_window_handle: xcb::Window,
    pub connection: Arc<xcb::Connection>,
    clients: Vec<TrayClient>,
}

impl TrayManager {
    pub fn init() -> Result<Self, TrayError> {
        let (connection, screen_num) = xcb::Connection::connect(None).map_err(|e| {
            TrayError::Connection { from: e }
        })?;

        let handle = Self::init_window(&connection, screen_num)?;

        Ok(Self {
            tray_window_handle: handle,
            connection: Arc::new(connection),
            clients: vec![],
        })
    }

    pub fn run_loop(&mut self) {
        loop {
            if let Some(ref event) = self.connection.wait_for_event() {
                let r = event.response_type() & !0x80;
                println!("{}:{}", event.response_type(), r);
                let handle_evt_result = match r {
                    xcb::DESTROY_NOTIFY => self.handle_destroy_notify(unsafe { xcb::cast_event(&event) }),
                    xcb::CLIENT_MESSAGE => self.handle_client_message(unsafe { xcb::cast_event(&event) }),
                    xcb::KEY_PRESS => self.handle_key_press(unsafe { xcb::cast_event(&event) }),

                    _ => continue,
                };

                match handle_evt_result {
                    Err(TrayError::Destroyed) => return,
                    Err(e) => println!("Error in message loop: {}", e),
                    Ok(_) => {}
                }
            }
        }
    }

    fn handle_destroy_notify(&mut self, evt: &xcb::DestroyNotifyEvent) -> Result<(), TrayError> {
        let destroyed_handle = evt.window();

        if destroyed_handle == self.tray_window_handle {
            println!("Tray was closed");
            Err(TrayError::Destroyed)
        } else {
            let (idx, client) = {
                let maybe_client = self.clients.iter().enumerate().find(|&(_, c): &(usize,
                                                                                    &TrayClient)| {
                    c.handle == evt.window()
                });

                if maybe_client.is_none() {
                    return Ok(());
                }

                let (idx, client) = maybe_client.unwrap();
                (idx, client.clone())
            };

            println!("Goodbye {}", client.cache_name.unwrap());
            self.clients.remove(idx);

            self.reflow_clients()?;

            Ok(())
        }
    }

    fn handle_client_message(&mut self, evt: &xcb::ClientMessageEvent) -> Result<(), TrayError> {
        assert_eq!(evt.format(), 32);
        let data = evt.data().data32();

        if data[0] == utils::get_atom(&self.connection, "WM_DELETE_WINDOW")? {
            xcb::unmap_window(&self.connection, self.tray_window_handle);
            self.connection.flush();
            return Ok(());
        }

        let client_handle = data[2];

        if evt.type_() != utils::get_atom(&self.connection, "_NET_SYSTEM_TRAY_OPCODE")? {
            return Ok(());
        }
        if data[1] == SYSTEM_TRAY_REQUEST_DOCK {
            let mut tray_client = TrayClient::new(client_handle);
            tray_client.store_cache_name(&self.connection)?;

            self.clients.push(tray_client);
            let tray_client = self.clients.last().unwrap();

            println!("name {}", tray_client.get_name(&self.connection).unwrap_or_else(|e| {
                println!("Couldn't get client name: {}", e);
                "".into()
            }));

            let values = [
                (
                    xcb::CW_EVENT_MASK,
                    xcb::EVENT_MASK_PROPERTY_CHANGE | xcb::EVENT_MASK_STRUCTURE_NOTIFY,
                ),
            ];
            xcb::change_window_attributes(&self.connection, client_handle, &values);
            let xembed_info = utils::map_get_property_reply(&xcb::get_property(
                &self.connection,
                false,
                tray_client.handle,
                utils::get_atom(&self.connection, "_XEMBED_INFO")?,
                xcb::GET_PROPERTY_TYPE_ANY,
                0,
                2 * 32,
            ))?;


            let xembed: &[u32] = xembed_info.value();
            println!("xembed version\t{}", xembed[0]);
            println!("xembed flags\t{}", xembed[1]);
            xcb::reparent_window(
                &self.connection,
                client_handle,
                self.tray_window_handle,
                0, 0,
            );

            xcb::configure_window(
                &self.connection,
                tray_client.handle,
                &[
                    (xcb::CONFIG_WINDOW_HEIGHT as u16, ICON_SIZE),
                    (xcb::CONFIG_WINDOW_WIDTH as u16, ICON_SIZE),
                ],
            );

            let xembed_msg = xcb::ClientMessageEvent::new(
                32,
                tray_client.handle,
                utils::get_atom(&self.connection, "_XEMBED")?,
                xcb::ClientMessageData::from_data32(
                    [
                        xcb::CURRENT_TIME,
                        utils::get_atom(&self.connection, "XEMBED_EMBEDDED_NOTIFY")?,
                        self.tray_window_handle,
                        xembed[0],
                        0,
                    ],
                ),
            );

            xcb::send_event(
                &self.connection,
                false,
                tray_client.handle,
                xcb::EVENT_MASK_NO_EVENT,
                &xembed_msg,
            );


            xcb::change_save_set(
                &self.connection,
                xcb::SET_MODE_INSERT as u8,
                tray_client.handle,
            );

            tray_client.map(&self.connection, true)?;

            self.reflow_clients()?;

            self.connection.flush();
        }

        Ok(())
    }

    fn handle_key_press(&mut self, evt: &xcb::KeyPressEvent) -> Result<(), TrayError> {
        match evt.detail() {
            24 | 9 => { // q or esc
                xcb::unmap_window_checked(&self.connection, self.tray_window_handle)
                    .request_check()
                    .map_err(|e| TrayError::GenericXcb { from: e })?
            }
            _ => {}
        }

        Ok(())
    }

    fn reflow_clients(&self) -> Result<(), TrayError> {
        for (idx, client) in self.clients.iter().enumerate() {
            client.reconfigure_placement(&self.connection, TRAY_LEFT_PAD + idx as u32 * ICON_SIZE, CLIENT_Y)?;
        }

        Ok(())
    }

    fn init_window(connection: &xcb::Connection, screen_num: i32) -> Result<xcb::Window, TrayError> {
        let setup = connection.get_setup();
        let screen = setup.roots().nth(screen_num as usize).ok_or_else(|| {
            TrayError::new(format!("Couldn't get screen {}", screen_num))
        })?;

        let tray_window_handle = connection.generate_id();

        let query_atom = utils::get_atom(connection, &format!("_NET_SYSTEM_TRAY_S{}", screen_num))?;

        let values = [
            (xcb::CW_BACK_PIXEL, screen.black_pixel()),
            (
                xcb::CW_EVENT_MASK,
                xcb::EVENT_MASK_EXPOSURE | xcb::EVENT_MASK_SUBSTRUCTURE_REDIRECT |
                    xcb::EVENT_MASK_STRUCTURE_NOTIFY | xcb::EVENT_MASK_KEY_PRESS,
            ),
        ];

        xcb::create_window(
            connection,
            xcb::COPY_FROM_PARENT as u8,
            tray_window_handle,
            screen.root(),
            0,
            0,
            300,
            15,
            0,
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            screen.root_visual(),
            &values,
        );

        // Set WM_CLASS
        utils::change_property_str(
            connection,
            tray_window_handle,
            xcb::ATOM_WM_CLASS,
            xcb::ATOM_STRING,
            "TrayOnDemand\0");

        utils::change_property_str(
            connection,
            tray_window_handle,
            xcb::ATOM_WM_NAME,
            xcb::ATOM_STRING,
            "Tray on Demand",
        );

        // Make window floating
        xcb::change_property(
            connection,
            xcb::PROP_MODE_REPLACE as u8,
            tray_window_handle,
            utils::get_atom(connection, "_NET_WM_WINDOW_TYPE")?,
            xcb::ATOM_ATOM,
            32,
            &[utils::get_atom(connection, "_NET_WM_WINDOW_TYPE_UTILITY")?],
        );

        let wm_delete_window_atom = utils::get_atom(connection, "WM_DELETE_WINDOW")?;

        // Request delete window messages
        xcb::change_property(
            connection,
            xcb::PROP_MODE_REPLACE as u8,
            tray_window_handle,
            utils::get_atom(connection, "WM_PROTOCOLS")?,
            xcb::ATOM_ATOM,
            32,
            &[wm_delete_window_atom],
        );

        xcb::map_window(connection, tray_window_handle);

        xcb::set_selection_owner(
            connection,
            tray_window_handle,
            query_atom,
            xcb::CURRENT_TIME,
        );

        connection.flush();

        Ok(tray_window_handle)
    }
}

impl Display for TrayManager {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "TrayManager {{handle: {}, clients: {:?}}}",
            self.tray_window_handle,
            self.clients
        )
    }
}
