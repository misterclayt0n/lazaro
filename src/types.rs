use std::fmt::Display;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use sqlx::prelude::Type;

#[derive(Clone, Debug, ValueEnum, Serialize, Deserialize, Type)]
#[sqlx(type_name = "TEXT")]
#[serde(rename_all = "kebab-case")]
pub enum Muscle {
    Biceps,
    Triceps,
    Forearms,
    Chest,
    Shoulders,
    Back,
    Quads,
    Hamstrings,
    Glutes,
    Calves,
    Abs,
}

impl Display for Muscle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Biceps => "biceps",
            Self::Triceps => "triceps",
            Self::Forearms => "forearms",
            Self::Chest => "chest",
            Self::Shoulders => "shoulders",
            Self::Back => "back",
            Self::Quads => "quads",
            Self::Hamstrings => "hamstrings",
            Self::Glutes => "glutes",
            Self::Calves => "calves",
            Self::Abs => "abs",
        };

        write!(f, "{}", s)
    }
}
