use std::ops::{Index, IndexMut};

use rand::{seq::SliceRandom, Rng};
use smallvec::SmallVec;
use smartstring::alias::String;

use crate::Side;

#[derive(Debug, Clone)]
// Add image support later.
#[non_exhaustive]
pub struct CardSide {
    pub text: SmallVec<[String; 1]>,
}

impl CardSide {
    pub fn any_text<R: Rng + ?Sized>(&self, rng: &mut R) -> Option<&str> {
        self.text.choose(rng).map(String::as_str)
    }

    pub fn matches_text(&self, rules: &TextMatching, text: &str) -> bool {
        self.text
            .iter()
            .any(|template| rules.test_match(template, text))
    }
}

#[derive(Debug, Clone)]
pub struct Flashcard {
    pub front: CardSide,
    pub back: CardSide,
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

#[derive(Debug, Clone)]
// Add image support later.
#[non_exhaustive]
pub struct McCard {
    pub question: CardSide,
    pub answer: CardSide,
    pub decoy_text: Vec<String>,
}

#[derive(Debug)]
pub struct Set {
    pub flashcards: Vec<Flashcard>,
    pub mc_cards: Vec<McCard>,
    pub properties: SetProperties,
}

impl Set {
    #[cfg(test)]
    pub(crate) fn example(properties: SetProperties) -> Set {
        use smallvec::smallvec;
        fn flashcard(front: &str, back: &str) -> Flashcard {
            Flashcard {
                front: CardSide {
                    text: smallvec![String::from(front)],
                },
                back: CardSide {
                    text: smallvec![String::from(back)],
                },
            }
        }

        fn mc_card<'a>(
            question: &str,
            answer: &str,
            decoys: impl IntoIterator<Item = &'a str>,
        ) -> McCard {
            McCard {
                question: CardSide {
                    text: smallvec![String::from(question)],
                },
                answer: CardSide {
                    text: smallvec![String::from(answer)],
                },
                decoy_text: decoys.into_iter().map(String::from).collect(),
            }
        }

        Set {
            flashcards: vec![
                flashcard("a", "0"),
                flashcard("b", "1"),
                flashcard("c", "2"),
                flashcard("d", "3"),
                flashcard("e", "4"),
                flashcard("f", "5"),
            ],
            mc_cards: vec![
                mc_card("0mc", "0answer", ["0decoy0", "0decoy1", "0decoy2"]),
                mc_card("1mc", "1answer", ["1decoy0", "1decoy1", "1decoy2"]),
                mc_card("2mc", "2answer", ["2decoy0", "2decoy1", "2decoy2"]),
                mc_card("3mc", "3answer", ["3decoy0", "3decoy1", "3decoy2"]),
            ],
            properties,
        }
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SetProperties {
    /// Whether to show front of card and ask player what was on the back.
    pub recall_back: RecallType,
    /// Whether to show back of card and ask player what was on the front.
    pub recall_front: RecallType,
    /// How to test if two strings are equal.
    pub text_matching: TextMatching,
}

impl Default for SetProperties {
    fn default() -> Self {
        Self {
            recall_back: RecallType::Mc,
            recall_front: RecallType::Mc,
            text_matching: TextMatching::default(),
        }
    }
}

/// How much of a side of a card does the player need to recall?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecallType {
    /// Does not need to recall.
    None,
    /// Must be able to select correct answer from a list.
    Mc,
    /// Must be able to recall text of answer.
    Text,
}

/// How to test if two strings are equal.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TextMatching {
    pub ignore_caps: bool,
}

impl Default for TextMatching {
    fn default() -> Self {
        Self { ignore_caps: true }
    }
}

impl TextMatching {
    pub fn test_match(&self, a: &str, b: &str) -> bool {
        let a = a.trim();
        let b = b.trim();
        if self.ignore_caps {
            unicase::eq(a, b)
        } else {
            a == b
        }
    }
}
