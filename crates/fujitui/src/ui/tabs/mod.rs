pub mod backup;
pub mod render;
pub mod simulation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Simulation,
    Render,
    Backup,
}

impl Tab {
    pub const ALL: [Self; 3] = [Self::Simulation, Self::Render, Self::Backup];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Simulation => "Simulations",
            Self::Render => "Rendering",
            Self::Backup => "Backups",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Simulation => 0,
            Self::Render => 1,
            Self::Backup => 2,
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Simulation => Self::Render,
            Self::Render => Self::Backup,
            Self::Backup => Self::Simulation,
        }
    }

    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Simulation => Self::Backup,
            Self::Render => Self::Simulation,
            Self::Backup => Self::Render,
        }
    }
}
