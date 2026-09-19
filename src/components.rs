use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ComponentKind {
    Localize,
    Install,
}

impl ComponentKind {
    pub const DEFAULT: [Self; 1] = [Self::Localize];

    pub fn instance(self) -> &'static dyn Component {
        match self {
            Self::Localize => &crate::tools::localizer::LOCALIZER,
            Self::Install => &crate::tools::installer::INSTALLER,
        }
    }
}

pub trait Component: std::fmt::Debug + Sync {
    fn command_name(&self) -> &'static str;

    fn service_descriptor(&self) -> Option<&'static ServiceDescriptor>;

    fn link_name(&self, style: LinkStyle) -> String {
        match style {
            LinkStyle::Default => self.command_name().to_owned(),
            LinkStyle::Prefix => format!("kako-{}", self.command_name()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LinkStyle {
    Default,
    Prefix,
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceDescriptor {
    pub component: &'static dyn Component,
    pub bundle_name: &'static str,
    pub bundle_identifier: &'static str,
    pub host_executable_name: &'static str,
    pub actions: &'static [ServiceActionDescriptor],
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceActionDescriptor {
    pub message: &'static str,
    pub menu_title: &'static str,
}
