use std::borrow::Cow;
use std::error;
use std::fmt;
use std::io;
use std::path::Path;

use crate::core::{ClassRegistry, Io, TryConvertMut};
use crate::error::{Error, RubyException};
use crate::ffi::InterpreterExtractError;
use crate::state::output::Output;
use crate::sys;
use crate::Artichoke;

impl Io for Artichoke {
    type Error = Error;

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
        P: AsRef<Path>,
    {
        let state = self.state.as_deref().ok_or_else(InterpreterExtractError::new)?;
        let path = path.as_ref();
        let contents = state.load_path_vfs.read_file(path)?;
        Ok(contents.into())
    }

    fn write_file(&mut self, path: &Path, buf: Cow<'static, [u8]>) -> Result<(), Self::Error> {
        let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
        let path = path.as_ref();
        state.load_path_vfs.write_file(path, buf.into())?;
        Ok(())
    }

    /// Writes the given bytes to the interpreter stdout stream.
    ///
    /// This implementation delegates to the underlying output strategy.
    ///
    /// # Errors
    ///
    /// If the output stream encounters an error, an error is returned.
    fn print(&mut self, message: &[u8]) -> Result<(), Self::Error> {
        let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
        state.output.write_stdout(message)?;
        Ok(())
    }

    /// Writes the given bytes to the interpreter stdout stream followed by a
    /// newline.
    ///
    /// This implementation delegates to the underlying output strategy.
    ///
    /// # Errors
    ///
    /// If the output stream encounters an error, an error is returned.
    fn puts(&mut self, message: &[u8]) -> Result<(), Self::Error> {
        let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
        state.output.write_stdout(message)?;
        state.output.write_stdout(b"\n")?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct IoError(io::Error);

impl From<io::Error> for IoError {
    fn from(err: io::Error) -> Self {
        Self(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::from(IoError::from(err))
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IOError: {}", self.0)
    }
}

impl error::Error for IoError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.0)
    }
}

impl RubyException for IoError {
    fn message(&self) -> Cow<'_, [u8]> {
        self.0.to_string().into_bytes().into()
    }

    fn name(&self) -> Cow<'_, str> {
        "IOError".into()
    }

    fn vm_backtrace(&self, interp: &mut Artichoke) -> Option<Vec<Vec<u8>>> {
        let _ = interp;
        None
    }

    fn as_mrb_value(&self, interp: &mut Artichoke) -> Option<sys::mrb_value> {
        let message = interp.try_convert_mut(self.message()).ok()?;
        let value = interp
            .new_instance::<spinoso_exception::IOError>(&[message])
            .ok()
            .flatten()?;
        Some(value.inner())
    }
}

impl From<IoError> for Error {
    fn from(exception: IoError) -> Self {
        let err: Box<dyn RubyException> = Box::new(exception);
        Self::from(err)
    }
}
