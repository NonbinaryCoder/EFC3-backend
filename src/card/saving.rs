use std::{
    io::{self, Write},
    path::Path,
};

use super::Set;

impl Set {
    /// Saves a set to a file.
    pub fn save(&self, file: impl AsRef<Path>) -> io::Result<()> {
        todo!()
    }

    /// Writes this set into the given writer.
    pub fn save_to_writer<W: Write>(&self, writer: W) -> io::Result<usize> {
        todo!()
    }
}
