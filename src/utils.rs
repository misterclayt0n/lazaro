use crate::models::OneRMFormula;

pub fn calculate_1rm(weight: f32, reps: u32, formula: OneRMFormula) -> f32 {
    match formula {
        OneRMFormula::Epley => weight * (1.0 + reps as f32 / 30.0),
        OneRMFormula::Brzycki => weight / (1.0278 - 0.0278 * reps as f32),
        OneRMFormula::Lombardi => weight * (reps as f32).powf(0.10),
        OneRMFormula::OConner => weight * (1.0 + 0.025 * reps as f32),
    }
}

pub fn format_duration(duration: chrono::Duration) -> String {
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
