use std::convert::Infallible;

use bollard::container::{Config, CreateContainerOptions};
use log::*;

#[derive(Clone)]
pub struct Docker {
    pub con: bollard::Docker,
}

impl Docker {
    pub fn init() -> Result<Docker, bollard::errors::Error> {
        let docker = bollard::Docker::connect_with_socket_defaults()?;

        Ok(Docker { con: docker })
    }
}
