use xcb;
use error::TrayError;

pub fn get_atom(connection: &xcb::Connection, name: &str) -> Result<xcb::Atom, TrayError> {
    Ok(
        xcb::intern_atom(&connection, false, name)
            .get_reply()
            .map_err(|e| TrayError::GenericXcb { from: e })?
            .atom(),
    )
}

pub fn get_property(
    connection: &xcb::Connection,
    window: xcb::Window,
    name: xcb::Atom,
    data_type: xcb::Atom,
) -> Result<xcb::GetPropertyReply, TrayError> {
    map_get_property_reply(&xcb::get_property(
        connection,
        false,
        window,
        name,
        data_type,
        0,
        1024,
    ))
}

pub fn map_get_property_reply(
    cookie: &xcb::GetPropertyCookie,
) -> Result<xcb::GetPropertyReply, TrayError> {
    cookie.get_reply().map_err(
        |e| TrayError::GenericXcb { from: e },
    )
}

pub fn change_property_str(
    connection: &xcb::Connection,
    window: xcb::Window,
    name: xcb::Atom,
    data_type: xcb::Atom,
    data: &str,
) {
    xcb::change_property(
        connection,
        xcb::PROP_MODE_REPLACE as u8,
        window,
        name,
        data_type,
        8,
        data.as_bytes(),
    );
}


pub fn append_property_str(
    connection: &xcb::Connection,
    window: xcb::Window,
    name: xcb::Atom,
    data_type: xcb::Atom,
    data: &str,
) {
    xcb::change_property(
        connection,
        xcb::PROP_MODE_APPEND as u8,
        window,
        name,
        data_type,
        8,
        data.as_bytes(),
    );
}
