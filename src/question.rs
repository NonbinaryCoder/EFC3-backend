use std::{borrow::Borrow, iter::FusedIterator, ops::Deref, ptr, slice};

use rand::{seq::SliceRandom, Rng};
use smallvec::SmallVec;

use crate::card::{Flashcard, McCard, RecallType, Set, Side};

/// Estimate of average max length of list returned by `Question::mc_answers`;
/// used to set size of smallvec.
const MC_LIST_LEN: usize = 6;
/// How many times to try to find enough decoys before giving up.
const FIND_DECOY_ATTEMPTS: usize = 24;

/// A question and answer.
///
/// Created by [`Set::questions`].
///
/// Not that this is NOT a card; some cards may generate as many as 2 qestions
/// while others may not generate any depending on settings used when converting
/// cards to questions.
#[derive(Debug)]
pub struct Question<'a> {
    pub(crate) set: &'a Set,
    pub(crate) ty: QuestionTy<'a>,
}

impl<'a> PartialEq for Question<'a> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.set, other.set) && self.ty == other.ty
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum QuestionTy<'a> {
    Flashcard {
        card: &'a Flashcard,
        // What side of the card to ask the player to recall.
        side: Side,
    },
    McCard {
        card: &'a McCard,
    },
}

impl<'a> Question<'a> {
    /// Question to ask the player.
    ///
    /// For [`Flashcard`]s this is the side of the card the player is not expected
    /// to recall.
    ///
    /// For [`McCard`]s this is the question.
    pub fn question<R: Rng + ?Sized>(&self, rng: &mut R) -> Option<&'a str> {
        match self.ty {
            QuestionTy::Flashcard { card, side } => card[!side].any_text(rng),
            QuestionTy::McCard { card } => card.question.any_text(rng),
        }
    }

    /// Whether or not a string is a correct answer to this question.
    ///
    /// Some questions may have more than one correct answer.
    pub fn is_correct_answer(&self, answer: &str) -> bool {
        match self.ty {
            QuestionTy::Flashcard { card, side } => {
                card[side].matches_text(self.set.flashcard_recall_settings(side), answer)
            }
            QuestionTy::McCard { card } => card.answer.matches_text(&self.set.recall_mc, answer),
        }
    }

    /// Returns a shuffled list containing the correct answer to this question
    /// and `count - 1` (or the max number of possible decoys if that is
    /// smaller) decoys.
    ///
    /// For [`Flashcard`]s decoys come from the other flashcards.
    ///
    /// For [`McCard`]s decoys come from provided decoys.
    pub fn mc_answers<R: Rng + ?Sized>(&self, count: usize, rng: &mut R) -> Option<McList<'a>> {
        // Remember to make sure this only returns one correct answer.
        match self.ty {
            QuestionTy::Flashcard { card, side } => {
                let answer_side = &card[side];
                // Calculate here and get out early.
                let correct_text = answer_side.any_text(rng)?;

                let flashcard_count = self.set.flashcards.len();
                let count = count.min(flashcard_count);

                let mut list = SmallVec::<[_; MC_LIST_LEN]>::with_capacity(count);
                for _ in 0..FIND_DECOY_ATTEMPTS {
                    let random_card = self
                        .set
                        .flashcards
                        .choose(rng)
                        .expect("Can't have card from list if list is empty");
                    // Get out early if accidently pick card question is about.
                    if ptr::eq(card, random_card) {
                        continue;
                    }

                    let Some(text) = random_card[side].any_text(rng) else {
                        continue;
                    };
                    if answer_side.matches_text(self.set.flashcard_recall_settings(side), text)
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
            QuestionTy::McCard { card } => {
                let correct_answer = card.answer.any_text(rng)?;
                let decoys = &card.decoys;

                let count = count.min(decoys.text_count() + 1);
                // If there are no decoys this card is probably a mistake.
                if count < 2 {
                    return None;
                }

                let mut decoys = decoys.choose_text(rng, count - 1);
                let correct_index = rng.gen_range(0..count);

                let mut list = SmallVec::with_capacity(count);
                list.extend(decoys.by_ref().take(correct_index));
                list.push(correct_answer);
                list.extend(decoys);

                Some(McList {
                    list,
                    correct_index,
                })
            }
        }
    }

    fn from_flashcard(card: &'a Flashcard, side: Side, set: &'a Set) -> Self {
        Question {
            set,
            ty: QuestionTy::Flashcard { card, side },
        }
    }

    fn from_mc_card(card: &'a McCard, set: &'a Set) -> Self {
        Question {
            set,
            ty: QuestionTy::McCard { card },
        }
    }
}

/// A list of decoys and one correct answer to a multiple choice question.
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
    /// The index into this where the correct answer is stored.
    pub fn correct_index(&self) -> usize {
        self.correct_index
    }

    /// The element of this list that is the correct answer.
    pub fn correct(&self) -> &'a str {
        self[self.correct_index()]
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Conditions {
    /// Whether to show front of card and ask player what was on the back.
    pub include_card_back: bool,
    /// Whether to show back of card and ask player what was on the front.
    pub include_card_front: bool,
    /// Whether to include multiple choice cards.
    pub include_mc: bool,
}

impl Conditions {
    pub const INCLUDE_ALL: Self = Self {
        include_card_back: true,
        include_card_front: true,
        include_mc: true,
    };

    pub const INCLUDE_NONE: Self = Self {
        include_card_back: false,
        include_card_front: false,
        include_mc: false,
    };
}

impl Default for Conditions {
    fn default() -> Self {
        Self {
            include_card_front: true,
            include_card_back: true,
            include_mc: true,
        }
    }
}

impl Set {
    /// Returns an iterator over all the questions that could be asked to prove
    /// knowledge of this set.  Allows for setting conditions to filter out
    /// questions.
    pub fn questions(&self, conditions: impl Borrow<Conditions>) -> Questions<'_> {
        self.questions_inner(conditions.borrow())
    }

    fn questions_inner(&self, conditions: &Conditions) -> Questions<'_> {
        Questions {
            set: self,
            flashcards_front: if conditions.include_card_front
                && self.recall_front.typ != RecallType::None
            {
                self.flashcards.iter()
            } else {
                [].iter()
            },
            flashcards_back: if conditions.include_card_back
                && self.recall_back.typ != RecallType::None
            {
                self.flashcards.iter()
            } else {
                [].iter()
            },
            mc_cards: if conditions.include_mc && self.recall_mc.typ != RecallType::None {
                self.mc_cards.iter()
            } else {
                [].iter()
            },
        }
    }
}

#[derive(Debug, Clone)]
/// An iterator over the [`Question`]s extracted from a [`Set`].
///
/// The order questions are returned in should not be depended on.
pub struct Questions<'a> {
    set: &'a Set,
    flashcards_back: slice::Iter<'a, Flashcard>,
    flashcards_front: slice::Iter<'a, Flashcard>,
    mc_cards: slice::Iter<'a, McCard>,
}

impl<'a> Iterator for Questions<'a> {
    type Item = Question<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.flashcards_back
            .next()
            .map(|card| Question::from_flashcard(card, Side::Back, self.set))
            .or_else(|| {
                self.flashcards_front
                    .next()
                    .map(|card| Question::from_flashcard(card, Side::Front, self.set))
            })
            .or_else(|| {
                self.mc_cards
                    .next()
                    .map(|card| Question::from_mc_card(card, self.set))
            })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }

    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.len()
    }

    fn for_each<F>(self, mut f: F)
    where
        Self: Sized,
        F: FnMut(Self::Item),
    {
        self.flashcards_back
            .map(|card| Question::from_flashcard(card, Side::Back, self.set))
            .for_each(&mut f);
        self.flashcards_front
            .map(|card| Question::from_flashcard(card, Side::Front, self.set))
            .for_each(&mut f);
        self.mc_cards
            .map(|card| Question::from_mc_card(card, self.set))
            .for_each(&mut f);
    }

    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let acc = self
            .flashcards_back
            .map(|card| Question::from_flashcard(card, Side::Back, self.set))
            .fold(init, &mut f);
        let acc = self
            .flashcards_front
            .map(|card| Question::from_flashcard(card, Side::Front, self.set))
            .fold(acc, &mut f);
        self.mc_cards
            .map(|card| Question::from_mc_card(card, self.set))
            .fold(acc, &mut f)
    }
}

impl<'a> ExactSizeIterator for Questions<'a> {
    fn len(&self) -> usize {
        self.flashcards_back.len() + self.flashcards_front.len() + self.mc_cards.len()
    }
}

impl<'a> FusedIterator for Questions<'a> {}

#[cfg(test)]
mod tests {
    use std::iter;

    use rand::SeedableRng;

    use super::*;

    const POSSIBLE_CONDITIONS: &[Conditions; 8] = &[
        Conditions::INCLUDE_NONE,
        Conditions {
            include_card_back: true,
            ..Conditions::INCLUDE_NONE
        },
        Conditions {
            include_card_front: true,
            ..Conditions::INCLUDE_NONE
        },
        Conditions {
            include_mc: true,
            ..Conditions::INCLUDE_NONE
        },
        Conditions {
            include_card_back: false,
            ..Conditions::INCLUDE_ALL
        },
        Conditions {
            include_card_front: false,
            ..Conditions::INCLUDE_ALL
        },
        Conditions {
            include_mc: false,
            ..Conditions::INCLUDE_ALL
        },
        Conditions::INCLUDE_ALL,
    ];

    #[test]
    fn questions_include_none() {
        let set = Set::example_recall_default();
        let questions = set.questions(&Conditions::INCLUDE_NONE);
        assert_eq!(questions.len(), 0);
    }

    #[test]
    fn questions_include_card_back() {
        let set = Set::example_recall_default();
        let questions = set
            .questions(Conditions {
                include_card_back: true,
                ..Conditions::INCLUDE_NONE
            })
            .collect::<Vec<_>>();
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
    fn questions_include_card_front() {
        let set = Set::example_recall_default();
        let questions = set
            .questions(Conditions {
                include_card_front: true,
                ..Conditions::INCLUDE_NONE
            })
            .collect::<Vec<_>>();
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
    fn questions_include_mc() {
        let set = Set::example_recall_default();
        let questions = set
            .questions(Conditions {
                include_mc: true,
                ..Conditions::INCLUDE_NONE
            })
            .collect::<Vec<_>>();
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
    fn questions_include_all() {
        let set = Set::example_recall_default();
        let questions = set.questions(Conditions::INCLUDE_ALL).collect::<Vec<_>>();
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
        let set = Set::example_recall_default();
        let question = set
            .questions(Conditions {
                include_card_front: true,
                ..Conditions::INCLUDE_NONE
            })
            .next()
            .unwrap();
        assert!(question.is_correct_answer("A "));
    }

    #[test]
    fn mc_answers_flashcard() {
        let set = Set::example_recall_default();
        let question = set
            .questions(Conditions {
                include_card_back: true,
                include_card_front: true,
                ..Conditions::INCLUDE_NONE
            })
            .next()
            .unwrap();
        let mut rng = rand::thread_rng();
        let answers = question.mc_answers(6, &mut rng).unwrap();
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
        let set = Set::example_recall_default();
        let question = set
            .questions(Conditions {
                include_mc: true,
                ..Conditions::INCLUDE_NONE
            })
            .next()
            .unwrap();
        let mut rng = rand::thread_rng();
        let answers = question.mc_answers(4, &mut rng).unwrap();
        assert_eq!(answers.len(), 4);
        assert!(answers.contains(&"0decoy0"));
        assert!(answers.contains(&"0decoy1"));
        assert!(answers.contains(&"0decoy2"));
        assert_eq!(answers.correct(), "0answer");
    }

    #[test]
    fn mc_answers_small_set() {
        let set = Set::example_recall_default();
        // Use deterministic RNG bc `Question::mc_answers` can return fewer
        // results than expected in unlucky situations.
        let mut rng = rand_chacha::ChaCha8Rng::from_seed(Default::default());

        let question = set
            .questions(Conditions {
                include_card_back: true,
                include_card_front: true,
                ..Conditions::INCLUDE_NONE
            })
            .next()
            .unwrap();
        let answers = question.mc_answers(256, &mut rng).unwrap();
        assert_eq!(answers.len(), 6);

        let question = set
            .questions(Conditions {
                include_mc: true,
                ..Conditions::INCLUDE_NONE
            })
            .next()
            .unwrap();
        let answers = question.mc_answers(256, &mut rng).unwrap();
        assert_eq!(answers.len(), 4);

        let mut questions = set.questions(Conditions::INCLUDE_ALL);
        let answers = questions.next().unwrap().mc_answers(256, &mut rng).unwrap();
        assert_eq!(answers.len(), 6);
        let answers = questions.last().unwrap().mc_answers(256, &mut rng).unwrap();
        assert_eq!(answers.len(), 4);
    }

    #[test]
    fn questions_correct_len() {
        let set = Set::example_recall_default();
        for (conditions, expected_count) in
            iter::zip(POSSIBLE_CONDITIONS, [0, 6, 6, 4, 10, 10, 12, 16])
        {
            assert_eq!(
                set.questions(conditions).len(),
                expected_count,
                "Failed at {:#?}",
                conditions
            );
        }
    }

    #[test]
    fn questions_len_matches_num_returned() {
        let set = Set::example_recall_default();
        for conditions in POSSIBLE_CONDITIONS {
            let questions = set.questions(conditions);
            let mut count = 0;
            for _ in questions.clone() {
                count += 1;
            }
            assert_eq!(questions.len(), count, "Failed at {:#?}", conditions);
        }
    }

    #[test]
    fn questions_for_each_matches_next() {
        let set = Set::example_recall_default();
        for conditions in POSSIBLE_CONDITIONS {
            let questions = set.questions(conditions);
            let mut next_questions = questions.clone();
            questions.for_each(|for_each| {
                let next = next_questions.next();
                assert_eq!(Some(for_each), next, "Failed at {:#?}", conditions);
            });
            assert!(
                next_questions.next().is_none(),
                "Failed at {:#?}",
                conditions
            );
        }
    }

    #[test]
    fn questions_fold() {
        let set = Set::example_recall_default();
        for conditions in POSSIBLE_CONDITIONS {
            let questions = set.questions(conditions);
            let count = questions.clone().fold(0, |acc, _| acc + 1);
            assert_eq!(questions.len(), count, "Failed at {:#?}", conditions);
        }
    }
}
