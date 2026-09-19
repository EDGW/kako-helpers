use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Component {
    Localize,
    Install,
}

impl Component {
    pub const DEFAULT: [Self; 1] = [Self::Localize];

    pub fn command_name(self) -> &'static str {
        match self {
            Self::Localize => "localize",
            Self::Install => "install",
        }
    }

    pub fn link_name(self, style: LinkStyle) -> String {
        match style {
            LinkStyle::Default => self.command_name().to_owned(),
            LinkStyle::Prefix => format!("kako-{}", self.command_name()),
        }
    }

    pub fn service_descriptor(self) -> Option<&'static ServiceDescriptor> {
        match self {
            Self::Localize => Some(&LOCALIZE_SERVICE),
            Self::Install => None,
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
    pub component: Component,
    pub bundle_name: &'static str,
    pub bundle_identifier: &'static str,
    pub host_executable_name: &'static str,
    pub localize_message: &'static str,
    pub localize_menu_title: &'static str,
    pub remove_message: &'static str,
    pub remove_menu_title: &'static str,
}

const LOCALIZE_SERVICE: ServiceDescriptor = ServiceDescriptor {
    component: Component::Localize,
    bundle_name: "KakoHelpersLocalize.service",
    bundle_identifier: "com.kako.helpers.localize-service",
    host_executable_name: "KakoHelpersLocalizeService",
    localize_message: "localizeFolder",
    localize_menu_title: "Localize Folder",
    remove_message: "removeLocalizedNames",
    remove_menu_title: "Remove Localized Names",
};
