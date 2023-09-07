//! A "simple" flashcards app.
//!
//! File format:
//!
//! The first line must be "EFC3 format \<version\>".  The second line may be in
//! the form "\<n\> terms".
//!
//! Settings for each side of a card and for multiple choice questions may be
//! specified by adding "@[card front]" "@[card back]" or "@\[mc\]" blocks.  The
//! recall property may be set to never, text, or multiple choice, defaulting
//! to multiple choice if unspecified.  The check caps property may be set to
//! true or false, defaulting to false.  Behavior when properties are repeated
//! is unspecified.
//!
//! Flashcard blocks are defined by a line starting with "\[card\]".  Any lines
//! below that starting with "F:" are used for the front of the card (so "F:
//! same") creates a card front with the text "same"; same for "B:" lines for
//! the back.
//!
//! Multiple choice blocks are defined by a line starting with "\[mc]\".  "Q:"
//! lines are for questions, "A:" lines are for answers, and "D:"" lines are
//! for decoys.
//!
//! Card text supports the following escapes: "\\n" for newline and "\\\\" for
//! backslash.

pub mod card;
pub mod question;
