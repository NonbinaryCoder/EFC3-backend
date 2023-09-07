use std::ops::{Index, IndexMut, Not};

use rand::{seq::SliceRandom, Rng};
use smallvec::{smallvec, SmallVec};
use smartstring::alias::String;

mod loading;
mod saving;

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

/// Text on the side of a [`Flashcard`], or as the question or answer to an
/// [`McCard`].
///
/// Stores multiple variants of text in order to show multiple variants of the
/// same question and accept multiple answers.
///
/// In the future may include images.
#[derive(Debug, Clone, PartialEq)]
pub struct CardSide {
    text: SmallVec<[String; 1]>,
}

impl CardSide {
    /// A `CardSide` containing no text or images.
    pub fn empty() -> Self {
        Self {
            text: SmallVec::new(),
        }
    }

    /// A `CardSide` containing the text given.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: smallvec![text.into()],
        }
    }

    /// A `CardSide` containing all the text given.
    pub fn new_multi<S: Into<String>>(texts: impl IntoIterator<Item = S>) -> Self {
        Self {
            text: texts.into_iter().map(Into::into).collect(),
        }
    }

    /// Add new text to this.
    pub fn push_text(&mut self, text: impl Into<String>) {
        self.text.push(text.into());
    }

    /// Remove and return the element at position `index`, shifting all elements
    /// after it to the left.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn remove_text(&mut self, index: usize) -> String {
        self.text.remove(index)
    }

    /// Returns a reference to the text at the index.
    pub fn get_text(&self, index: usize) -> Option<&str> {
        self.text.get(index).map(AsRef::as_ref)
    }

    /// Returns a mutable reference to the text at the index.
    pub fn get_text_mut(&mut self, index: usize) -> Option<&mut String> {
        self.text.get_mut(index)
    }

    /// Returns a random piece text from this to use as a question or answer.
    pub fn any_text<R: Rng + ?Sized>(&self, rng: &mut R) -> Option<&str> {
        self.text.choose(rng).map(String::as_str)
    }

    /// Returns true if the provided text matches any of the text in this by the
    /// rules provided.
    pub fn matches_text(&self, rules: &RecallSettings, text: &str) -> bool {
        self.text
            .iter()
            .any(|template| rules.test_match(template, text))
    }
}

impl From<String> for CardSide {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<std::string::String> for CardSide {
    fn from(value: std::string::String) -> Self {
        Self::new(value)
    }
}

impl<'a> From<&'a str> for CardSide {
    fn from(value: &'a str) -> Self {
        Self::new(value)
    }
}

/// A list of decoys used by a multiple choice question.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoys {
    text: SmallVec<[String; 3]>,
}

impl Decoys {
    /// No decoys.
    pub fn empty() -> Self {
        Self {
            text: SmallVec::new(),
        }
    }

    /// Add an aditional decoy.
    pub fn push_text(&mut self, text: impl Into<String>) {
        self.text.push(text.into());
    }

    /// Picks `count` random pieces of text from this to use as decoys and
    /// returns them.
    ///
    /// Does not return repeat elements.
    pub(crate) fn choose_text<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        count: usize,
    ) -> impl Iterator<Item = &str> {
        self.text.choose_multiple(rng, count).map(AsRef::as_ref)
    }

    /// Returns the number of text decoys this has.
    pub fn text_count(&self) -> usize {
        self.text.len()
    }
}

impl<S: Into<String>> FromIterator<S> for Decoys {
    fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
        Self {
            text: iter.into_iter().map(Into::into).collect(),
        }
    }
}

/// A flashcard with text on the front and back.
#[derive(Debug, Clone, PartialEq)]
pub struct Flashcard {
    pub front: CardSide,
    pub back: CardSide,
}

impl Flashcard {
    /// A `Flashcard` with no text.
    pub fn blank() -> Self {
        Self {
            front: CardSide::empty(),
            back: CardSide::empty(),
        }
    }

    /// A `Flashcard` with the provided front and back text.
    pub fn new(front: impl Into<String>, back: impl Into<String>) -> Self {
        Self {
            front: front.into().into(),
            back: back.into().into(),
        }
    }
}

impl Index<Side> for Flashcard {
    type Output = CardSide;

    fn index(&self, index: Side) -> &Self::Output {
        match index {
            Side::Front => &self.front,
            Side::Back => &self.back,
        }
    }
}

impl IndexMut<Side> for Flashcard {
    fn index_mut(&mut self, index: Side) -> &mut Self::Output {
        match index {
            Side::Front => &mut self.front,
            Side::Back => &mut self.back,
        }
    }
}

/// A multiple choice question.
#[derive(Debug, Clone, PartialEq)]
pub struct McCard {
    pub question: CardSide,
    pub answer: CardSide,
    pub decoys: Decoys,
}

impl McCard {
    /// A multiple choice question without any question or answer or decoys
    /// written.
    pub fn blank() -> Self {
        Self {
            question: CardSide::empty(),
            answer: CardSide::empty(),
            decoys: Decoys::empty(),
        }
    }
}

/// A set of [`Flashcard`]s and [`McCard`]s.
///
/// Contains information about how the player should be asked to recall various
/// parts of cards.
#[derive(Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Set {
    /// Rules for how the player should prove they know what is on the back of a
    /// [`Flashcard`] when the front is shown.
    pub recall_back: RecallSettings,
    /// Rules for how the player should prove they know what is on the front of
    /// a [`Flashcard`] when the back is shown.
    pub recall_front: RecallSettings,
    /// Rules for how the player should prove they know what the answer to a
    /// multiple choice question ([`McCard`]) is when the question is shown.
    pub recall_mc: RecallSettings,
    pub flashcards: Vec<Flashcard>,
    pub mc_cards: Vec<McCard>,
}

impl Set {
    pub(crate) fn flashcard_recall_settings(&self, side: Side) -> &RecallSettings {
        match side {
            Side::Front => &self.recall_front,
            Side::Back => &self.recall_back,
        }
    }

    #[cfg(test)]
    pub(crate) fn example_recall_default() -> Self {
        Self::example(
            RecallSettings::default(),
            RecallSettings::default(),
            RecallSettings::default(),
        )
    }

    #[cfg(test)]
    pub(crate) fn example(
        recall_front: RecallSettings,
        recall_back: RecallSettings,
        recall_mc: RecallSettings,
    ) -> Set {
        fn mc_card<'a>(
            question: &str,
            answer: &str,
            decoys: impl IntoIterator<Item = &'a str>,
        ) -> McCard {
            McCard {
                question: CardSide::new(question),
                answer: CardSide::new(answer),
                decoys: decoys.into_iter().collect(),
            }
        }

        Set {
            flashcards: vec![
                Flashcard::new("a", "0"),
                Flashcard::new("b", "1"),
                Flashcard::new("c", "2"),
                Flashcard::new("d", "3"),
                Flashcard::new("e", "4"),
                Flashcard::new("f", "5"),
            ],
            mc_cards: vec![
                mc_card("0mc", "0answer", ["0decoy0", "0decoy1", "0decoy2"]),
                mc_card("1mc", "1answer", ["1decoy0", "1decoy1", "1decoy2"]),
                mc_card("2mc", "2answer", ["2decoy0", "2decoy1", "2decoy2"]),
                mc_card("3mc", "3answer", ["3decoy0", "3decoy1", "3decoy2"]),
            ],
            recall_front,
            recall_back,
            recall_mc,
        }
    }
}

/// How does the player have to prove they remember what is on the card?
///
/// Additional options will likely be added to this in the future.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RecallSettings {
    /// Should the player be asked to select correct answer from a list?  Type
    /// in the answer?
    pub typ: RecallType,
    /// Does capitalization in the answer matter?
    pub check_caps: bool,
}

impl RecallSettings {
    fn test_match(&self, a: &str, b: &str) -> bool {
        let a = a.trim();
        let b = b.trim();
        if self.check_caps {
            a == b
        } else {
            unicase::eq(a, b)
        }
    }
}

impl Default for RecallSettings {
    fn default() -> Self {
        Self {
            typ: RecallType::Mc,
            check_caps: false,
        }
    }
}

/// How much of a side of a card does the player need to recall?
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecallType {
    /// Does not need to recall.
    None,
    /// Must be able to select correct answer from a list.
    Mc,
    /// Must be able to type in text of answer.
    Text,
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::card::loading::Version;

    use super::*;

    const SERIALIZED_SET: &str = "EFC3 format 1.0.0
10 terms

@[card front]
recall: text
check caps: true

@[card back]
recall: never
check caps: false

@[mc]
recall: multiple choice
check caps: false

[card]
F: a
B: 0

[card]
F: b
B: 1

[card]
F: c
B: 2

[card]
F: d
B: 3

[card]
F: e
B: 4

[card]
F: f
B: 5

[mc]
Q: 0mc
A: 0answer
D: 0decoy0
D: 0decoy1
D: 0decoy2

[mc]
Q: 1mc
A: 1answer
D: 1decoy0
D: 1decoy1
D: 1decoy2

[mc]
Q: 2mc
A: 2answer
D: 2decoy0
D: 2decoy1
D: 2decoy2

[mc]
Q: 3mc
A: 3answer
D: 3decoy0
D: 3decoy1
D: 3decoy2
";

    #[test]
    fn set_serialize() {
        let mut buf = Vec::new();
        Set::example(
            RecallSettings {
                typ: RecallType::Mc,
                check_caps: true,
            },
            RecallSettings {
                typ: RecallType::None,
                check_caps: false,
            },
            RecallSettings {
                typ: RecallType::Mc,
                check_caps: false,
            },
        )
        .save_to_writer(&mut buf)
        .unwrap();
        assert_eq!(buf, SERIALIZED_SET.as_bytes());
    }

    #[test]
    fn set_deserialize() {
        let (set, version) = Set::load_from_reader(Cursor::new(SERIALIZED_SET)).unwrap();
        assert_eq!(
            set,
            Set::example(
                RecallSettings {
                    typ: RecallType::Text,
                    check_caps: true,
                },
                RecallSettings {
                    typ: RecallType::None,
                    check_caps: false,
                },
                RecallSettings {
                    typ: RecallType::Mc,
                    check_caps: false,
                },
            )
        );
        assert_eq!(version, Some(Version::new(1, 0, 0)))
    }
}
