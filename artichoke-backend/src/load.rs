use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::Path;

use artichoke_core::eval::Eval;
use artichoke_core::file::File;
use artichoke_core::load::{LoadSources, Loaded, Required};
use artichoke_core::prelude::Io;
use scolapasta_path::os_str_to_bytes;
use spinoso_exception::LoadError;

use crate::error::Error;
use crate::ffi::InterpreterExtractError;
use crate::Artichoke;

const RUBY_EXTENSION: &str = "rb";

impl LoadSources for Artichoke {
    type Artichoke = Self;
    type Error = Error;
    type Exception = Error;

    fn def_file_for_type<P, T>(&mut self, path: P) -> Result<(), Self::Error>
    where
        P: AsRef<Path>,
        T: File<Artichoke = Self::Artichoke, Error = Self::Exception>,
    {
        let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
        let path = path.as_ref();
        state.load_path_vfs.register_extension(path, T::require)?;
        Ok(())
    }

    fn def_rb_source_file<P, T>(&mut self, path: P, contents: T) -> Result<(), Self::Error>
    where
        P: AsRef<Path>,
        T: Into<Cow<'static, [u8]>>,
    {
        let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
        let path = path.as_ref();
        state.load_path_vfs.write_file(path, contents.into())?;
        Ok(())
    }

    fn resolve_source_path<P>(&self, path: P) -> Result<Option<Vec<u8>>, Self::Error>
    where
        P: AsRef<Path>,
    {
        let state = self.state.as_deref().ok_or_else(InterpreterExtractError::new)?;
        let path = path.as_ref();
        if let Some(path) = state.load_path_vfs.resolve_file(path) {
            return Ok(Some(path));
        }
        // If the given path did not end in `.rb`, try again with a `.rb` file
        // extension.
        if !matches!(path.extension(), Some(ext) if *ext == *OsStr::new(RUBY_EXTENSION)) {
            let mut path = path.to_owned();
            path.set_extension(RUBY_EXTENSION);
            return Ok(state.load_path_vfs.resolve_file(&path));
        }
        Ok(None)
    }

    fn source_is_file<P>(&self, path: P) -> Result<bool, Self::Error>
    where
        P: AsRef<Path>,
    {
        let state = self.state.as_deref().ok_or_else(InterpreterExtractError::new)?;
        let path = path.as_ref();
        if state.load_path_vfs.is_file(path) {
            return Ok(true);
        }
        // If the given path did not end in `.rb`, try again with a `.rb` file
        // extension.
        if !matches!(path.extension(), Some(ext) if *ext == *OsStr::new(RUBY_EXTENSION)) {
            let mut path = path.to_owned();
            path.set_extension(RUBY_EXTENSION);
            if state.load_path_vfs.is_file(&path) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn load_source<P>(&mut self, path: P) -> Result<Loaded, Self::Error>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
        // Require Rust `File` first because an File may define classes and
        // modules with `LoadSources` and Ruby files can require arbitrary
        // other files, including some child sources that may depend on these
        // module definitions.
        if let Some(hook) = state.load_path_vfs.get_extension(path) {
            // dynamic, Rust-backed `File` require
            hook(self)?;
        }
        let contents = self
            .read_file(path)
            .map_err(|_| {
                let mut message = b"cannot load such file".to_vec();
                if let Ok(bytes) = os_str_to_bytes(path.as_os_str()) {
                    message.extend_from_slice(b" -- ");
                    message.extend_from_slice(bytes);
                }
                LoadError::from(message)
            })?
            .into_owned();
        self.eval(contents.as_ref())?;
        Ok(Loaded::Success)
    }

    fn require_source<P>(&mut self, path: P) -> Result<Required, Self::Error>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let mut alternate_path;
        let path = {
            let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
            // If a file is already required, short circuit.
            if let Some(true) = state.load_path_vfs.is_required(path) {
                return Ok(Required::AlreadyRequired);
            }
            // Require Rust `File` first because an File may define classes and
            // modules with `LoadSources` and Ruby files can require arbitrary
            // other files, including some child sources that may depend on these
            // module definitions.
            match state.load_path_vfs.get_extension(path) {
                Some(hook) => {
                    // dynamic, Rust-backed `File` require
                    hook(self)?;
                    path
                }
                None if matches!(path.extension(), Some(ext) if *ext == *OsStr::new(RUBY_EXTENSION)) => path,
                None => {
                    alternate_path = path.to_owned();
                    alternate_path.set_extension(RUBY_EXTENSION);
                    // If a file is already required, short circuit.
                    if let Some(true) = state.load_path_vfs.is_required(&alternate_path) {
                        return Ok(Required::AlreadyRequired);
                    }
                    if let Some(hook) = state.load_path_vfs.get_extension(&alternate_path) {
                        // dynamic, Rust-backed `File` require
                        hook(self)?;
                    } else {
                        // Try to load the source at the given path
                        if let Ok(contents) = self.read_file(path) {
                            let contents = contents.into_owned();
                            self.eval(&contents)?;
                            let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
                            state.load_path_vfs.mark_required(path)?;
                            return Ok(Required::Success);
                        }
                        // else proceed with the alternate path
                    }
                    // This ensures that if we load the hook at an alternate
                    // path, we use that alternate path to load the Ruby source.
                    &alternate_path
                }
            }
        };
        let contents = self.read_file(path)?.into_owned();
        self.eval(contents.as_ref())?;
        let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
        state.load_path_vfs.mark_required(path)?;
        Ok(Required::Success)
    }
}

#[cfg(test)]
mod tests {
    use artichoke_core::load::{Loaded, Required};
    use bstr::ByteSlice;

    use crate::test::prelude::*;

    const NON_IDEMPOTENT_LOAD: &[u8] = br#"
module LoadSources
  class Counter
    attr_reader :c

    def initialize(c)
      @c = c
    end

    def inc!
      @c += 1
    end

    def self.instance
      @instance ||= new(10)
    end
  end
end

LoadSources::Counter.instance.inc!
    "#;

    #[test]
    fn load_has_no_memory() {
        let mut interp = interpreter();
        interp.def_rb_source_file("counter.rb", NON_IDEMPOTENT_LOAD).unwrap();

        let result = interp.load_source("./counter.rb").unwrap();
        assert_eq!(result, Loaded::Success);
        let count = interp
            .eval(b"LoadSources::Counter.instance.c")
            .unwrap()
            .try_convert_into::<usize>(&interp)
            .unwrap();
        assert_eq!(count, 11);

        // `Kernel#load` has no memory and will always execute
        let result = interp.load_source("./counter.rb").unwrap();
        assert_eq!(result, Loaded::Success);
        let count = interp
            .eval(b"LoadSources::Counter.instance.c")
            .unwrap()
            .try_convert_into::<usize>(&interp)
            .unwrap();
        assert_eq!(count, 12);
    }

    #[test]
    fn load_has_no_memory_and_ignores_loaded_features() {
        let mut interp = interpreter();
        interp.def_rb_source_file("counter.rb", NON_IDEMPOTENT_LOAD).unwrap();

        let result = interp.require_source("./counter.rb").unwrap();
        assert_eq!(result, Required::Success);
        let count = interp
            .eval(b"LoadSources::Counter.instance.c")
            .unwrap()
            .try_convert_into::<usize>(&interp)
            .unwrap();
        assert_eq!(count, 11);

        let result = interp.require_source("./counter.rb").unwrap();
        assert_eq!(result, Required::AlreadyRequired);

        let result = interp.load_source("./counter.rb").unwrap();
        assert_eq!(result, Loaded::Success);
        let count = interp
            .eval(b"LoadSources::Counter.instance.c")
            .unwrap()
            .try_convert_into::<usize>(&interp)
            .unwrap();
        assert_eq!(count, 12);

        // `Kernel#load` has no memory and will always execute
        let result = interp.load_source("./counter.rb").unwrap();
        assert_eq!(result, Loaded::Success);
        let count = interp
            .eval(b"LoadSources::Counter.instance.c")
            .unwrap()
            .try_convert_into::<usize>(&interp)
            .unwrap();
        assert_eq!(count, 13);
    }

    #[test]
    fn load_does_not_discover_paths_from_loaded_features() {
        let mut interp = interpreter();
        interp.def_rb_source_file("counter.rb", NON_IDEMPOTENT_LOAD).unwrap();

        let result = interp.require_source("./counter").unwrap();
        assert_eq!(result, Required::Success);
        let count = interp
            .eval(b"LoadSources::Counter.instance.c")
            .unwrap()
            .try_convert_into::<usize>(&interp)
            .unwrap();
        assert_eq!(count, 11);

        let exc = interp.load_source("./counter").unwrap_err();
        assert_eq!(exc.message().as_bstr(), b"cannot load such file -- ./counter".as_bstr());
        assert_eq!(exc.name(), "LoadError");
    }
}
