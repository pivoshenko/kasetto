#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Skills,
    Mcps,
}

impl Tab {
    pub(super) fn label(self) -> &'static str {
        match self {
            Tab::Skills => "Skills",
            Tab::Mcps => "MCPs",
        }
    }
}
