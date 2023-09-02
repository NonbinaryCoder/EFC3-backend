use std::ops::Not;

pub mod card;
pub mod question;

/// A side of a flashcard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Front,
    Back,
}

impl Not for Side {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Side::Front => Side::Back,
            Side::Back => Side::Front,
        }
    }
}
