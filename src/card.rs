use std::ops::{Index, IndexMut};

use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use smartstring::alias::String;

use crate::Side;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Add image support later.
#[non_exhaustive]
pub struct CardSide {
    pub text: SmallVec<[String; 1]>,
}

impl CardSide {
    pub fn any_text<R: Rng + ?Sized>(&self, rng: &mut R) -> Option<&str> {
        self.text.choose(rng).map(String::as_str)
    }

    pub fn matches_text(&self, rules: &MatchingRules, text: &str) -> bool {
        self.text
            .iter()
            .any(|template| rules.test_match(template, text))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Add image support later.
#[non_exhaustive]
pub struct McCard {
    pub question: CardSide,
    pub answer: CardSide,
    pub decoy_text: SmallVec<[String; 4]>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Set {
    pub properties: SetProperties,
    pub flashcards: Vec<Flashcard>,
    pub mc_cards: Vec<McCard>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SetProperties {
    /// Whether to show front of card and ask player what was on the back.
    pub recall_back: RecallType,
    /// Whether to show back of card and ask player what was on the front.
    pub recall_front: RecallType,
    /// How to test if two strings are equal.
    pub matching_rules: MatchingRules,
}

impl Default for SetProperties {
    fn default() -> Self {
        Self {
            recall_back: RecallType::Mc,
            recall_front: RecallType::Mc,
            matching_rules: MatchingRules::default(),
        }
    }
}

/// How much of a side of a card does the player need to recall?
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecallType {
    /// Does not need to recall.
    #[serde(rename = "never")]
    None,
    /// Must be able to select correct answer from a list.
    #[serde(rename = "multiple choice")]
    Mc,
    /// Must be able to recall text of answer.
    #[serde(rename = "text")]
    Text,
}

/// How to test if two strings are equal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MatchingRules {
    pub ignore_caps: bool,
}

impl Default for MatchingRules {
    fn default() -> Self {
        Self { ignore_caps: true }
    }
}

impl MatchingRules {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SERIALIZED_SET: &str = "properties:
  recall_back: never
  recall_front: multiple choice
  matching_rules:
    ignore_caps: true
flashcards:
- front:
    text:
    - a
  back:
    text:
    - '0'
- front:
    text:
    - b
  back:
    text:
    - '1'
- front:
    text:
    - c
  back:
    text:
    - '2'
- front:
    text:
    - d
  back:
    text:
    - '3'
- front:
    text:
    - e
  back:
    text:
    - '4'
- front:
    text:
    - f
  back:
    text:
    - '5'
mc_cards:
- question:
    text:
    - 0mc
  answer:
    text:
    - 0answer
  decoy_text:
  - 0decoy0
  - 0decoy1
  - 0decoy2
- question:
    text:
    - 1mc
  answer:
    text:
    - 1answer
  decoy_text:
  - 1decoy0
  - 1decoy1
  - 1decoy2
- question:
    text:
    - 2mc
  answer:
    text:
    - 2answer
  decoy_text:
  - 2decoy0
  - 2decoy1
  - 2decoy2
- question:
    text:
    - 3mc
  answer:
    text:
    - 3answer
  decoy_text:
  - 3decoy0
  - 3decoy1
  - 3decoy2
";

    #[test]
    fn set_serialize() {
        let serialized = serde_yaml::to_string(&Set::example(SetProperties {
            recall_back: RecallType::None,
            ..Default::default()
        }))
        .unwrap();
        assert_eq!(serialized, SERIALIZED_SET);
    }

    #[test]
    fn set_deserialize() {
        let deserialized: Set = serde_yaml::from_str(SERIALIZED_SET).unwrap();
        assert_eq!(
            deserialized,
            Set::example(SetProperties {
                recall_back: RecallType::None,
                ..Default::default()
            })
        );
    }
}
