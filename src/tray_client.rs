use std::str;
use xcb;

use utils;
use error::TrayError;

#[derive(Debug, Clone)]
pub struct TrayClient {
    pub handle: u32,
    pub cache_name: Option<String>,
}

impl TrayClient {
    pub fn new(handle: u32) -> Self {
        Self {
            handle,
            cache_name: None,
        }
    }

    pub fn map(&self, connection: &xcb::Connection, state: bool) -> Result<(), TrayError> {
        if state {
            xcb::map_window_checked(connection, self.handle).request_check()
        } else {
            xcb::unmap_window_checked(connection, self.handle).request_check()
        }.map_err(|e| TrayError::GenericXcb { from: e })
    }

    pub fn store_cache_name(&mut self, connection: &xcb::Connection) -> Result<(), TrayError> {
        self.cache_name = Some(self.get_name(connection)?);

        Ok(())
    }

    pub fn reconfigure_placement(&self, connection: &xcb::Connection, x : u32, y: u32) -> Result<(), TrayError> {
        xcb::configure_window_checked(&connection, self.handle, &[
            (xcb::CONFIG_WINDOW_X as u16, x),
            (xcb::CONFIG_WINDOW_Y as u16, y)
        ]).request_check().map_err(|e| TrayError::GenericXcb { from: e })?;
        Ok(())
    }

    pub fn get_name(&self, connection: &xcb::Connection) -> Result<String, TrayError> {
        utils::get_property(
            connection,
            self.handle,
            utils::get_atom(&connection, "_NET_WM_NAME")?,
            utils::get_atom(&connection, "UTF8_STRING")?,
        ).map(|r| {
            str::from_utf8(r.value())
                .expect("Atom with type UTF&_STRING wasn't valid utf-8")
                .into()
        })
    }
}
