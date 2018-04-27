use xcb;
use zmq;

#[derive(Debug, Fail)]
pub enum TrayError {
    #[fail(display = "Generic error: {}", name)]
    Generic { name: String },
    #[fail(display = "Generic XCB error: {}", from)]
    GenericXcb { from: xcb::GenericError },
    #[fail(display = "Connection error: {}", from)]
    Connection { from: xcb::ConnError },
    #[fail(display = "ZMQ error: {}", from)]
    ZMQ { from: zmq::Error },
    #[fail(display = "Destroyed")]
    Destroyed,
}

impl TrayError {
    pub fn new<S: Into<String>>(name: S) -> Self {
        TrayError::Generic { name: name.into() }
    }
}
