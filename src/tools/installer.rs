use crate::components::{Component, ServiceDescriptor};

#[derive(Debug)]
pub struct Installer;

pub static INSTALLER: Installer = Installer;

impl Component for Installer {
    fn command_name(&self) -> &'static str {
        "install"
    }

    fn service_descriptor(&self) -> Option<&'static ServiceDescriptor> {
        None
    }
}
