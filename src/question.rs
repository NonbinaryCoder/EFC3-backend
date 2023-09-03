use std::{borrow::Borrow, ops::Deref};

use rand::{seq::SliceRandom, Rng};
use smallvec::SmallVec;

use crate::{
    card::{RecallType, Set},
    Side,
};

/// Estimate of average max length of list returned by `Question::mc_answers`;
/// used to set size of smallvec.
const MC_LIST_LEN: usize = 6;
/// How many times to try to find enough decoys before giving up.
const FIND_DECOY_ATTEMPTS: usize = 24;

/// A question and answer.
///
/// Not that this is NOT a card; some cards may generate as many as 2 qestions
/// while others may not generate any depending on settings used when converting
/// cards to questions.
#[derive(Debug)]
pub struct Question<'a> {
    pub(crate) cards: &'a Set,
    pub(crate) ty: QuestionTy,
}

#[derive(Debug)]
pub(crate) enum QuestionTy {
    Flashcard {
        // Index of the `Flashcard` this question is from.
        index: usize,
        // What side of the card to ask the player to recall.
        side: Side,
    },
    McCard {
        // Index of the `McCard` this question is from.
        index: usize,
    },
}

impl<'a> Question<'a> {
    /// Question to ask the player.
    ///
    /// For flashcards this is the side of the card the player is not expected
    /// to recall.  For multiple choice cards this is the question.
    pub fn question<R: Rng + ?Sized>(&self, rng: &mut R) -> Option<&'a str> {
        match self.ty {
            QuestionTy::Flashcard { index, side, .. } => {
                self.cards.flashcards[index][!side].any_text(rng)
            }
            QuestionTy::McCard { index } => self.cards.mc_cards[index].question.any_text(rng),
        }
    }

    /// Whether or not a string is a correct answer to this question.
    ///
    /// Some questions may have more than one correct answer.
    pub fn is_correct_answer(&self, answer: &str) -> bool {
        match self.ty {
            QuestionTy::Flashcard { index, side, .. } => self.cards.flashcards[index][side]
                .matches_text(&self.cards.properties.matching_rules, answer),
            QuestionTy::McCard { index } => self.cards.mc_cards[index]
                .answer
                .matches_text(&self.cards.properties.matching_rules, answer),
        }
    }

    /// Returns a shuffled list containing the correct answer to this question
    /// and `count - 1` (or the max number of possible decoys if that is
    /// smaller) decoys.
    pub fn mc_answers<R: Rng + ?Sized>(&self, count: usize, rng: &mut R) -> Option<McList<'a>> {
        // Remember to make sure this only returns one correct answer.
        match self.ty {
            QuestionTy::Flashcard { index, side } => {
                let card = &self.cards.flashcards[index];
                let answer_side = &card[side];
                // Calculate here and get out early.
                let correct_text = answer_side.any_text(rng)?;

                let flashcard_count = self.cards.flashcards.len();
                let count = count.min(flashcard_count);

                let mut list = SmallVec::<[_; MC_LIST_LEN]>::with_capacity(count);
                for _ in 0..FIND_DECOY_ATTEMPTS {
                    let random_index = rng.gen_range(0..flashcard_count);
                    dbg!(random_index);
                    if random_index == index {
                        continue;
                    }

                    let Some(text) = self.cards.flashcards[random_index][side].any_text(rng) else {
                        continue;
                    };
                    if answer_side.matches_text(&self.cards.properties.matching_rules, text)
                        || list.contains(&text)
                    {
                        continue;
                    }

                    list.push(text);
                    if list.len() == count - 1 {
                        break;
                    }
                }

                if list.is_empty() {
                    return None;
                }
                let correct_index = rng.gen_range(0..list.len());
                list.insert(correct_index, correct_text);

                Some(McList {
                    list,
                    correct_index,
                })
            }
            QuestionTy::McCard { index } => {
                let card = &self.cards.mc_cards[index];
                let correct_answer = card.answer.any_text(rng)?;
                let decoy_text = &card.decoy_text;

                let count = count.min(decoy_text.len() + 1);
                // If there are no decoys this card is probably a mistake.
                if count < 2 {
                    return None;
                }

                let mut decoys = decoy_text.choose_multiple(rng, count - 1);
                let correct_index = rng.gen_range(0..count);

                let mut list = SmallVec::with_capacity(count);
                list.extend(decoys.by_ref().take(correct_index).map(|s| s.as_str()));
                list.push(correct_answer);
                list.extend(decoys.map(|s| s.as_str()));

                Some(McList {
                    list,
                    correct_index,
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct McList<'a> {
    list: SmallVec<[&'a str; MC_LIST_LEN]>,
    correct_index: usize,
}

impl<'a> Deref for McList<'a> {
    type Target = [&'a str];

    fn deref(&self) -> &Self::Target {
        &self.list
    }
}

impl<'a> McList<'a> {
    pub fn correct_index(&self) -> usize {
        self.correct_index
    }

    pub fn correct(&self) -> &'a str {
        self[self.correct_index()]
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Conditions {
    /// Whether to show front of card and ask player what was on the back.
    pub recall_back: bool,
    /// Whether to show back of card and ask player what was on the front.
    pub recall_front: bool,
    /// Whether to include multiple choice cards.
    pub include_mc: bool,
}

impl Conditions {
    pub const ALL_TRUE: Self = Self {
        recall_back: true,
        recall_front: true,
        include_mc: true,
    };

    pub const ALL_FALSE: Self = Self {
        recall_back: false,
        recall_front: false,
        include_mc: false,
    };
}

impl Default for Conditions {
    fn default() -> Self {
        Self {
            recall_front: true,
            recall_back: true,
            include_mc: true,
        }
    }
}

impl Set {
    pub fn to_questions(&self, conditions: impl Borrow<Conditions>) -> Vec<Question<'_>> {
        self.to_questions_inner(conditions.borrow())
    }

    fn to_questions_inner(&self, conditions: &Conditions) -> Vec<Question<'_>> {
        let mut questions = Vec::new();

        let recall_back = conditions.recall_back && self.properties.recall_back != RecallType::None;
        let recall_front =
            conditions.recall_front && self.properties.recall_front != RecallType::None;

        let mut extract_questions = |side| {
            for index in 0..self.flashcards.len() {
                questions.push(Question {
                    cards: self,
                    ty: QuestionTy::Flashcard { index, side },
                })
            }
        };

        if recall_back {
            extract_questions(Side::Back);
        }
        if recall_front {
            extract_questions(Side::Front);
        }

        if conditions.include_mc {
            for index in 0..self.mc_cards.len() {
                questions.push(Question {
                    cards: self,
                    ty: QuestionTy::McCard { index },
                })
            }
        }

        questions
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use crate::card::{MatchingRules, SetProperties};

    use super::*;

    /// An object implementing `rand::Rng`, but that is not actually random to
    /// ensure consistency of tests
    #[derive(Debug, Clone)]
    struct NotRng(u64);

    impl rand::RngCore for NotRng {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }

        fn next_u64(&mut self) -> u64 {
            self.0 += 1;
            self.0
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for item in dest {
                *item = self.next_u64() as u8;
            }
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            for item in dest {
                *item = self.next_u64() as u8;
            }
            Ok(())
        }
    }

    #[test]
    fn questions_all_conditions_false() {
        let set = Set::example(SetProperties::default());
        let questions = set.to_questions(&Conditions::ALL_FALSE);
        assert_eq!(questions.len(), 0);
    }

    #[test]
    fn questions_recall_back_only() {
        let set = Set::example(SetProperties::default());
        let questions = set.to_questions(Conditions {
            recall_back: true,
            ..Conditions::ALL_FALSE
        });
        let mut rng = rand::thread_rng();
        assert_eq!(questions[0].question(&mut rng), Some("a"));
        assert!(questions[0].is_correct_answer("0"));
        assert_eq!(questions[1].question(&mut rng), Some("b"));
        assert!(questions[1].is_correct_answer("1"));
        assert_eq!(questions[2].question(&mut rng), Some("c"));
        assert!(questions[2].is_correct_answer("2"));
        assert_eq!(questions[3].question(&mut rng), Some("d"));
        assert!(questions[3].is_correct_answer("3"));
        assert_eq!(questions[4].question(&mut rng), Some("e"));
        assert!(questions[4].is_correct_answer("4"));
        assert_eq!(questions[5].question(&mut rng), Some("f"));
        assert!(questions[5].is_correct_answer("5"));
        assert_eq!(questions.len(), 6);
    }

    #[test]
    fn questions_recall_front_only() {
        let set = Set::example(SetProperties::default());
        let questions = set.to_questions(Conditions {
            recall_front: true,
            ..Conditions::ALL_FALSE
        });
        let mut rng = rand::thread_rng();
        assert_eq!(questions[0].question(&mut rng), Some("0"));
        assert!(questions[0].is_correct_answer("a"));
        assert_eq!(questions[1].question(&mut rng), Some("1"));
        assert!(questions[1].is_correct_answer("b"));
        assert_eq!(questions[2].question(&mut rng), Some("2"));
        assert!(questions[2].is_correct_answer("c"));
        assert_eq!(questions[3].question(&mut rng), Some("3"));
        assert!(questions[3].is_correct_answer("d"));
        assert_eq!(questions[4].question(&mut rng), Some("4"));
        assert!(questions[4].is_correct_answer("e"));
        assert_eq!(questions[5].question(&mut rng), Some("5"));
        assert!(questions[5].is_correct_answer("f"));
        assert_eq!(questions.len(), 6);
    }

    #[test]
    fn questions_recall_mc_only() {
        let set = Set::example(SetProperties::default());
        let questions = set.to_questions(Conditions {
            include_mc: true,
            ..Conditions::ALL_FALSE
        });
        let mut rng = rand::thread_rng();
        assert_eq!(questions[0].question(&mut rng), Some("0mc"));
        assert!(questions[0].is_correct_answer("0answer"));
        assert_eq!(questions[1].question(&mut rng), Some("1mc"));
        assert!(questions[1].is_correct_answer("1answer"));
        assert_eq!(questions[2].question(&mut rng), Some("2mc"));
        assert!(questions[2].is_correct_answer("2answer"));
        assert_eq!(questions[3].question(&mut rng), Some("3mc"));
        assert!(questions[3].is_correct_answer("3answer"));
        assert_eq!(questions.len(), 4);
    }

    #[test]
    fn questions_all_conditions_true() {
        let set = Set::example(SetProperties::default());
        let questions = set.to_questions(Conditions::ALL_TRUE);
        let mut rng = rand::thread_rng();

        assert_eq!(questions[0].question(&mut rng), Some("a"));
        assert!(questions[0].is_correct_answer("0"));
        assert_eq!(questions[1].question(&mut rng), Some("b"));
        assert!(questions[1].is_correct_answer("1"));
        assert_eq!(questions[2].question(&mut rng), Some("c"));
        assert!(questions[2].is_correct_answer("2"));
        assert_eq!(questions[3].question(&mut rng), Some("d"));
        assert!(questions[3].is_correct_answer("3"));
        assert_eq!(questions[4].question(&mut rng), Some("e"));
        assert!(questions[4].is_correct_answer("4"));
        assert_eq!(questions[5].question(&mut rng), Some("f"));
        assert!(questions[5].is_correct_answer("5"));

        assert_eq!(questions[6].question(&mut rng), Some("0"));
        assert!(questions[6].is_correct_answer("a"));
        assert_eq!(questions[7].question(&mut rng), Some("1"));
        assert!(questions[7].is_correct_answer("b"));
        assert_eq!(questions[8].question(&mut rng), Some("2"));
        assert!(questions[8].is_correct_answer("c"));
        assert_eq!(questions[9].question(&mut rng), Some("3"));
        assert!(questions[9].is_correct_answer("d"));
        assert_eq!(questions[10].question(&mut rng), Some("4"));
        assert!(questions[10].is_correct_answer("e"));
        assert_eq!(questions[11].question(&mut rng), Some("5"));
        assert!(questions[11].is_correct_answer("f"));

        assert_eq!(questions[12].question(&mut rng), Some("0mc"));
        assert!(questions[12].is_correct_answer("0answer"));
        assert_eq!(questions[13].question(&mut rng), Some("1mc"));
        assert!(questions[13].is_correct_answer("1answer"));
        assert_eq!(questions[14].question(&mut rng), Some("2mc"));
        assert!(questions[14].is_correct_answer("2answer"));
        assert_eq!(questions[15].question(&mut rng), Some("3mc"));
        assert!(questions[15].is_correct_answer("3answer"));
        assert_eq!(questions.len(), 16);
    }

    #[test]
    fn match_text_ignore_caps() {
        let set = Set::example(SetProperties::default());
        let questions = set.to_questions(Conditions {
            recall_front: true,
            ..Conditions::ALL_FALSE
        });
        assert!(questions[0].is_correct_answer("A"));
    }

    #[test]
    fn match_text_with_caps() {
        let set = Set::example(SetProperties {
            matching_rules: MatchingRules {
                ignore_caps: false,
                ..Default::default()
            },
            ..Default::default()
        });
        let questions = set.to_questions(Conditions {
            recall_front: true,
            ..Conditions::ALL_FALSE
        });
        assert!(questions[0].is_correct_answer("a "));
    }

    #[test]
    fn mc_answers_flashcard() {
        let set = Set::example(SetProperties::default());
        let questions = set.to_questions(Conditions {
            recall_back: true,
            recall_front: true,
            ..Conditions::ALL_FALSE
        });
        let mut rng = rand::thread_rng();
        let answers = questions[0].mc_answers(6, &mut rng).unwrap();
        assert_eq!(answers.len(), 6);
        assert!(answers.contains(&"1"));
        assert!(answers.contains(&"2"));
        assert!(answers.contains(&"3"));
        assert!(answers.contains(&"4"));
        assert!(answers.contains(&"5"));
        assert_eq!(answers.correct(), "0");
    }

    #[test]
    fn mc_answers_mc_card() {
        let set = Set::example(SetProperties::default());
        let questions = set.to_questions(Conditions {
            include_mc: true,
            ..Conditions::ALL_FALSE
        });
        let mut rng = rand::thread_rng();
        let answers = questions[0].mc_answers(4, &mut rng).unwrap();
        assert_eq!(answers.len(), 4);
        assert!(answers.contains(&"0decoy0"));
        assert!(answers.contains(&"0decoy1"));
        assert!(answers.contains(&"0decoy2"));
        assert_eq!(answers.correct(), "0answer");
    }

    #[test]
    fn mc_answers_small_set() {
        let set = Set::example(SetProperties::default());
        // Use deterministic RNG bc `Question::mc_answers` can return fewer
        // results than expected in unlucky situations.
        let mut rng = rand_chacha::ChaCha8Rng::from_seed(Default::default());

        let questions = set.to_questions(Conditions {
            recall_back: true,
            recall_front: true,
            ..Conditions::ALL_FALSE
        });
        let answers = questions[0].mc_answers(256, &mut rng).unwrap();
        assert_eq!(answers.len(), 6);

        let questions = set.to_questions(Conditions {
            include_mc: true,
            ..Conditions::ALL_FALSE
        });
        let answers = questions[0].mc_answers(256, &mut rng).unwrap();
        assert_eq!(answers.len(), 4);

        let questions = set.to_questions(Conditions::ALL_TRUE);
        let answers = questions[0].mc_answers(256, &mut rng).unwrap();
        assert_eq!(answers.len(), 6);
        let answers = questions[questions.len() - 1]
            .mc_answers(256, &mut rng)
            .unwrap();
        assert_eq!(answers.len(), 4);
    }
}
