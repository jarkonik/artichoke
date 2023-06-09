//! I/O read and write APIs.

use std::{fs::File, os::fd::RawFd, path::Path};

use alloc::borrow::Cow;

/// Perform I/O external to the interpreter.
pub trait Io {
    /// Concrete error type for errors encountered when reading and writing.
    type Error;

    fn file_from_raw_fd(&self, fd: RawFd) -> Result<File, Self::Error>;

    /// Retrieve file contents for a source file.
    ///
    /// Query the underlying virtual file system for the file contents of the
    /// source file at `path`.
    ///
    /// # Errors
    ///
    /// If the underlying file system is inaccessible, an error is returned.
    ///
    /// If reads to the underlying file system fail, an error is returned.
    ///
    /// If `path` does not point to a source file, an error is returned.
    fn read_file<P>(&self, path: P) -> Result<Cow<'_, [u8]>, Self::Error>
    where
        P: AsRef<Path>;

    fn write_file(&mut self, path: &Path, buf: Cow<'static, [u8]>) -> Result<(), Self::Error>;

    /// Writes the given bytes to the interpreter stdout stream.
    ///
    /// # Errors
    ///
    /// If the output stream encounters an error, an error is returned.
    fn print(&mut self, message: &[u8]) -> Result<(), Self::Error>;

    /// Writes the given bytes to the interpreter stdout stream followed by a
    /// newline.
    ///
    /// The default implementation uses two calls to [`print`].
    ///
    /// # Errors
    ///
    /// If the output stream encounters an error, an error is returned.
    ///
    /// [`print`]: Self::print
    fn puts(&mut self, message: &[u8]) -> Result<(), Self::Error> {
        self.print(message)?;
        self.print(b"\n")?;
        Ok(())
    }
}
