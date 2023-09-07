use std::{
    fmt::{self, Display},
    fs::File,
    io::{self, Read},
    path::Path,
};

use nom::{
    bytes::complete::{tag, take_till1},
    character::{
        complete::{self as cc, char, newline, space0},
        streaming::not_line_ending,
    },
    combinator::{opt, value},
    error::ParseError,
    sequence::{delimited, pair, separated_pair, terminated},
    Finish, Parser,
};
use smartstring::alias::String;

use super::{Flashcard, McCard, RecallSettings, RecallType, Set, Side};

type IResult<I, O> = nom::IResult<I, O, Error>;

type Span<'a> = nom_locate::LocatedSpan<&'a str>;

impl Set {
    /// Loads a set from a file.
    pub fn load(file: impl AsRef<Path>) -> Result<(Self, Option<Version>)> {
        Self::load_from_reader(File::open(file)?)
    }

    /// Constructs a set by reading from the given reader.
    pub fn load_from_reader<R: Read>(mut reader: R) -> Result<(Self, Option<Version>)> {
        fn inner(s: &str) -> Result<(Set, Option<Version>)> {
            let s = Span::new(s);
            separated_pair(opt(first_line), opt(second_line), body)
                .map(|(version, set)| (set, version))
                .parse(s)
                .map(|(_, ret)| ret)
                .finish()
        }

        let mut buf = std::string::String::new();
        reader.read_to_string(&mut buf)?;
        buf.push('\n');
        inner(&buf)
    }
}

fn first_line(s: Span<'_>) -> IResult<Span<'_>, Version> {
    delimited(tag("EFC3 format "), Version::parse, newline)(s)
}

fn second_line(s: Span<'_>) -> IResult<Span<'_>, Option<u32>> {
    opt(terminated(cc::u32, pair(tag(" terms"), newline)))(s)
}

fn body(mut s: Span<'_>) -> IResult<Span<'_>, Set> {
    let mut set = Set::default();
    while let Ok((rem, line)) = terminated(not_line_ending::<_, Error>, opt(newline))(s) {
        s = rem;
        s = match line.trim() {
            "@[card front]" => set.recall_front.update(s)?.0,
            "@[card back]" => set.recall_back.update(s)?.0,
            "@[mc]" => set.recall_mc.update(s)?.0,
            "[card]" => {
                let (s, card) = Flashcard::parse(s)?;
                set.flashcards.push(card);
                s
            }
            "[mc]" => {
                let (s, card) = McCard::parse(s)?;
                set.mc_cards.push(card);
                s
            }
            _ => continue,
        };
    }
    Ok((s, set))
}

fn property_separator(s: Span<'_>) -> IResult<Span<'_>, ()> {
    value((), pair(char(':'), space0))(s)
}

fn property_value(s: Span<'_>) -> IResult<Span<'_>, (Span<'_>, Span<'_>)> {
    pair(
        terminated(
            take_till1(|ch| matches!(ch, ':' | '\n')),
            property_separator,
        ),
        terminated(take_till1(|ch| ch == '\n'), newline),
    )(s)
}

impl RecallSettings {
    fn update<'a>(&mut self, mut s: Span<'a>) -> IResult<Span<'a>, ()> {
        while let Ok((rem, (property, value))) = property_value(s) {
            s = rem;
            let value = value.trim();
            match property.trim() {
                "recall" => {
                    self.typ = RecallType::from_str(value).ok_or(nom::Err::Failure(
                        Error::InvalidType {
                            line: property.location_line(),
                            expected: RecallType::EXPECTED_VALUES,
                        },
                    ))?
                }
                "check caps" => {
                    self.check_caps = value.parse().map_err(|_| {
                        nom::Err::Failure(Error::InvalidType {
                            line: property.location_line(),
                            expected: "{ true | false }",
                        })
                    })?
                }
                _ => {}
            }
        }
        Ok((s, ()))
    }
}

impl RecallType {
    const EXPECTED_VALUES: &str = "{ never | multiple choice | text}";

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "never" => Some(Self::None),
            "multiple choice" => Some(Self::Mc),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

impl Flashcard {
    fn parse(mut s: Span<'_>) -> IResult<Span<'_>, Self> {
        let mut card = Self::blank();
        while let Ok((rem, (property, value))) = property_value(s) {
            s = rem;
            let side = match property.trim() {
                "F" => Side::Front,
                "B" => Side::Back,
                _ => continue,
            };
            card[side].push_text(string_from_escaped(value.trim_start()));
        }
        Ok((s, card))
    }
}

impl McCard {
    fn parse(mut s: Span<'_>) -> IResult<Span<'_>, Self> {
        let mut card = Self::blank();
        while let Ok((rem, (property, value))) = property_value(s) {
            s = rem;
            let value = string_from_escaped(value.trim_start());
            match property.trim() {
                "Q" => card.question.push_text(value),
                "A" => card.answer.push_text(value),
                "D" => card.decoys.push_text(value),
                _ => {}
            }
        }
        Ok((s, card))
    }
}

fn string_from_escaped(s: &str) -> String {
    let mut buf = String::new();
    let mut chars = s.chars();
    while let Some(char) = chars.next() {
        if char == '\\' {
            match chars.next() {
                Some('\\') => buf.push('\\'),
                Some('n') => buf.push('\n'),
                Some(' ') => buf.push(' '),
                Some(_) => {}
                None => buf.push('\\'),
            }
        } else {
            buf.push(char);
        }
    }
    buf
}

#[derive(Debug)]
pub enum Error {
    /// Error opening file or reading from reader.
    Io(io::Error),
    /// Parser failed.
    ParseError { line: u32 },
    /// Attempt to assign incorrect type to property.
    InvalidType { line: u32, expected: &'static str },
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {e}"),
            Error::ParseError { line } => write!(f, "Parser error on line {line}"),
            Error::InvalidType { line, expected } => {
                write!(
                    f,
                    "Property on line {line} expects value of type {expected}"
                )
            }
        }
    }
}

impl<'a> ParseError<Span<'a>> for Error {
    fn from_error_kind(input: Span<'a>, _: nom::error::ErrorKind) -> Self {
        Self::ParseError {
            line: input.location_line(),
        }
    }

    fn append(_: Span<'a>, _: nom::error::ErrorKind, other: Self) -> Self {
        other
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
}

impl Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    fn parse(s: Span<'_>) -> IResult<Span<'_>, Self> {
        separated_pair(
            separated_pair(cc::u32, char('.'), cc::u32),
            char('.'),
            cc::u32,
        )
        .map(|((major, minor), patch)| Self {
            major,
            minor,
            patch,
        })
        .parse(s)
    }
}

#[cfg(test)]
mod tests {
    use crate::card::CardSide;

    use super::*;

    #[test]
    fn first_line_version() {
        let (rem, version) = first_line("EFC3 format 1.2.4\n".into()).unwrap();
        assert_eq!(version, Version::new(1, 2, 4));
        assert!(rem.is_empty());
    }

    #[test]
    fn property_value_test() {
        let (rem, (property, value)) = property_value("  prop  :    val \n".into()).unwrap();
        assert_eq!(property.trim(), "prop");
        assert_eq!(value.trim(), "val");
        assert!(rem.is_empty());
    }

    #[test]
    fn recall_settings_recall() {
        let mut rules = RecallSettings {
            typ: RecallType::Text,
            ..Default::default()
        };

        let (rem, ()) = rules.update("recall: never\n".into()).unwrap();
        assert_eq!(rules.typ, RecallType::None);
        assert!(rem.is_empty());

        let (rem, ()) = rules.update("recall: multiple choice\n".into()).unwrap();
        assert_eq!(rules.typ, RecallType::Mc);
        assert!(rem.is_empty());

        let (rem, ()) = rules.update(" recall : text \n".into()).unwrap();
        assert_eq!(rules.typ, RecallType::Text);
        assert!(rem.is_empty());
    }

    #[test]
    #[allow(clippy::bool_assert_comparison)]
    fn recall_settings_check_caps() {
        let mut rules = RecallSettings {
            check_caps: true,
            ..Default::default()
        };

        let (rem, ()) = rules.update("check caps: false\n".into()).unwrap();
        assert_eq!(rules.check_caps, false);
        assert!(rem.is_empty());

        let (rem, ()) = rules.update(" check caps : true \n".into()).unwrap();
        assert_eq!(rules.check_caps, true);
        assert!(rem.is_empty());
    }

    #[test]
    fn flashcard_single_texts() {
        let (rem, card) = Flashcard::parse("F: a\n B : 0\n".into()).unwrap();
        assert_eq!(card, Flashcard::new("a", "0"));
        assert!(rem.is_empty());
    }

    #[test]
    fn flashcard_multiple_texts() {
        let (rem, card) = Flashcard::parse("F: a\nF: A\nB: 0\nB: )\n".into()).unwrap();
        assert_eq!(
            card,
            Flashcard {
                front: CardSide::new_multi(["a", "A"]),
                back: CardSide::new_multi(["0", ")"]),
            }
        );
        assert!(rem.is_empty());
    }

    #[test]
    fn mc_card_single_texts() {
        let (rem, card) = McCard::parse("Q: 0mc\n A : 0answer\nD: 0decoy0\n".into()).unwrap();
        assert_eq!(
            card,
            McCard {
                question: "0mc".into(),
                answer: "0answer".into(),
                decoys: ["0decoy0"].into_iter().collect(),
            }
        );
        assert!(rem.is_empty());
    }

    #[test]
    fn mc_card_multiple_texts() {
        let (rem, card) = McCard::parse(
            "Q: 0mc\nQ: 0MC\nA: 0answer\nA: 0ANSWER\nD: 0decoy0\nD: 0decoy1\nD: 0decoy2\n".into(),
        )
        .unwrap();
        assert_eq!(
            card,
            McCard {
                question: CardSide::new_multi(["0mc", "0MC"]),
                answer: CardSide::new_multi(["0answer", "0ANSWER"]),
                decoys: ["0decoy0", "0decoy1", "0decoy2"].into_iter().collect(),
            }
        );
        assert!(rem.is_empty());
    }
}
