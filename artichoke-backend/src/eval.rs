use std::ffi::OsStr;
use std::path::Path;

use artichoke_core::prelude::Io;
use scolapasta_path::os_str_to_bytes;
use spinoso_exception::{ArgumentError, Fatal, LoadError};

use crate::core::{Eval, Parser};
use crate::error::Error;
use crate::ffi::InterpreterExtractError;
use crate::state::parser::Context;
use crate::sys;
use crate::sys::protect;
use crate::value::Value;
use crate::Artichoke;
use crate::{exception_handler, RubyException};

impl Eval for Artichoke {
    type Value = Value;

    type Error = Error;

    fn eval(&mut self, code: &[u8]) -> Result<Self::Value, Self::Error> {
        let result = unsafe {
            let state = self.state.as_deref_mut().ok_or_else(InterpreterExtractError::new)?;
            let parser = state.parser.as_mut().ok_or_else(InterpreterExtractError::new)?;
            let context: *mut sys::mrbc_context = parser.context_mut();
            self.with_ffi_boundary(|mrb| protect::eval(mrb, context, code))?
        };

        let result = result.map(Value::from).map_err(Value::from);

        match result {
            Ok(value) if value.is_unreachable() => {
                // Unreachable values are internal to the mruby interpreter and
                // interacting with them via the C API is unspecified and may
                // result in a segfault.
                //
                // See: https://github.com/mruby/mruby/issues/4460
                emit_fatal_warning!("eval returned an unreachable Ruby value");
                Err(Fatal::from("eval returned an unreachable Ruby value").into())
            }
            Ok(value) => Ok(self.protect(value)),
            Err(exception) => {
                let exception = self.protect(exception);
                Err(exception_handler::last_error(self, exception)?)
            }
        }
    }

    fn eval_os_str(&mut self, code: &OsStr) -> Result<Self::Value, Self::Error> {
        let code = os_str_to_bytes(code)?;
        self.eval(code)
    }

    fn eval_file(&mut self, file: &Path) -> Result<Self::Value, Self::Error> {
        let context = Context::new(os_str_to_bytes(file.as_os_str())?.to_vec())
            .ok_or_else(|| ArgumentError::with_message("path name contains null byte"))?;
        self.push_context(context)?;
        let code = self
            .read_file(file)
            .map_err(|err| {
                let mut message = b"ruby: ".to_vec();
                message.extend_from_slice(err.message().as_ref());
                if let Ok(bytes) = os_str_to_bytes(file.as_os_str()) {
                    message.extend_from_slice(b" -- ");
                    message.extend_from_slice(bytes);
                }
                LoadError::from(message)
            })?
            .into_owned();
        let result = self.eval(code.as_slice());
        self.pop_context()?;
        result
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::ffi::OsStr;
    #[cfg(unix)]
    use std::os::unix::ffi::OsStrExt;
    use std::path::Path;

    use bstr::ByteSlice;

    use crate::test::prelude::*;

    #[test]
    fn root_eval_context() {
        let mut interp = interpreter();
        let result = interp.eval(b"__FILE__").unwrap();
        let result = result.try_convert_into_mut::<&str>(&mut interp).unwrap();
        assert_eq!(result, "(eval)");
    }

    #[test]
    fn context_is_restored_after_eval() {
        let mut interp = interpreter();
        let context = Context::new(&b"context.rb"[..]).unwrap();
        interp.push_context(context).unwrap();
        interp.eval(b"15").unwrap();
        let context = interp.peek_context().unwrap();
        let filename = context.unwrap().filename();
        assert_eq!(filename.as_bstr(), b"context.rb".as_bstr());
    }

    #[test]
    fn root_context_is_not_pushed_after_eval() {
        let mut interp = interpreter();
        interp.eval(b"15").unwrap();
        let context = interp.peek_context().unwrap();
        assert!(context.is_none());
    }

    mod nested {
        use crate::test::prelude::*;

        #[derive(Debug)]
        struct NestedEval;

        unsafe extern "C" fn nested_eval_file(mrb: *mut sys::mrb_state, _slf: sys::mrb_value) -> sys::mrb_value {
            unwrap_interpreter!(mrb, to => guard);
            let result = if let Ok(value) = guard.eval(b"__FILE__") {
                value
            } else {
                Value::nil()
            };
            result.inner()
        }

        impl File for NestedEval {
            type Artichoke = Artichoke;

            type Error = Error;

            fn require(interp: &mut Artichoke) -> Result<(), Self::Error> {
                let spec = module::Spec::new(interp, "NestedEval", qed::const_cstr_from_str!("NestedEval\0"), None)?;
                module::Builder::for_spec(interp, &spec)
                    .add_self_method("file", nested_eval_file, sys::mrb_args_none())?
                    .define()?;
                interp.def_module::<Self>(spec)?;
                Ok(())
            }
        }

        #[test]
        #[should_panic]
        // this test is known broken
        fn eval_context_is_a_stack() {
            let mut interp = interpreter();
            interp.def_file_for_type::<_, NestedEval>("nested_eval.rb").unwrap();
            let code = br#"require 'nested_eval'; NestedEval.file"#;
            let result = interp.eval(code).unwrap();
            let result = result.try_convert_into_mut::<&str>(&mut interp).unwrap();
            assert_eq!(result, "/src/lib/nested_eval.rb");
        }
    }

    #[test]
    fn eval_with_context() {
        let mut interp = interpreter();

        let context = Context::new(b"source.rb".as_ref()).unwrap();
        interp.push_context(context).unwrap();
        let result = interp.eval(b"__FILE__").unwrap();
        let result = result.try_convert_into_mut::<&str>(&mut interp).unwrap();
        assert_eq!(result, "source.rb");
        interp.pop_context().unwrap();

        let context = Context::new(b"source.rb".as_ref()).unwrap();
        interp.push_context(context).unwrap();
        let result = interp.eval(b"__FILE__").unwrap();
        let result = result.try_convert_into_mut::<&str>(&mut interp).unwrap();
        assert_eq!(result, "source.rb");
        interp.pop_context().unwrap();

        let context = Context::new(b"main.rb".as_ref()).unwrap();
        interp.push_context(context).unwrap();
        let result = interp.eval(b"__FILE__").unwrap();
        let result = result.try_convert_into_mut::<&str>(&mut interp).unwrap();
        assert_eq!(result, "main.rb");
        interp.pop_context().unwrap();
    }

    #[test]
    fn unparseable_code_returns_err_syntax_error() {
        let mut interp = interpreter();
        let err = interp.eval(b"'a").unwrap_err();
        assert_eq!("SyntaxError", err.name().as_ref());
    }

    #[test]
    fn interpreter_is_usable_after_syntax_error() {
        let mut interp = interpreter();
        let err = interp.eval(b"'a").unwrap_err();
        assert_eq!("SyntaxError", err.name().as_ref());
        // Ensure interpreter is usable after evaling unparseable code
        let result = interp.eval(b"'a' * 10 ").unwrap();
        let result = result.try_convert_into_mut::<&str>(&mut interp).unwrap();
        assert_eq!(result, "a".repeat(10));
    }

    #[test]
    fn file_magic_constant() {
        let file = if cfg!(windows) {
            "c:/artichoke/virtual_root/src/lib/source.rb"
        } else {
            "/artichoke/virtual_root/src/lib/source.rb"
        };
        let mut interp = interpreter();
        interp
            .def_rb_source_file("source.rb", &b"def file; __FILE__; end"[..])
            .unwrap();
        let result = interp.eval(b"require 'source'; file").unwrap();
        let result = result.try_convert_into_mut::<&str>(&mut interp).unwrap();
        assert_eq!(result, file);
    }

    #[test]
    fn file_not_persistent() {
        let mut interp = interpreter();
        interp
            .def_rb_source_file("source.rb", &b"def file; __FILE__; end"[..])
            .unwrap();
        let result = interp.eval(b"require 'source'; __FILE__").unwrap();
        let result = result.try_convert_into_mut::<&str>(&mut interp).unwrap();
        assert_eq!(result, "(eval)");
    }

    #[test]
    fn return_syntax_error() {
        let mut interp = interpreter();
        interp
            .def_rb_source_file("fail.rb", &b"def bad; 'as'.scan(; end"[..])
            .unwrap();
        let err = interp.eval(b"require 'fail'").unwrap_err();
        assert_eq!("SyntaxError", err.name().as_ref());
    }

    #[test]
    fn eval_file_error_file_not_found() {
        let mut interp = interpreter();
        let err = interp.eval_file(Path::new("no/such/file.rb")).unwrap_err();
        assert_eq!("LoadError", err.name().as_ref());
        assert_eq!(
            b"ruby: file not found in virtual file system -- no/such/file.rb",
            err.message().as_ref()
        );
    }

    #[test]
    #[cfg(unix)]
    fn eval_file_error_invalid_path() {
        let mut interp = interpreter();
        let err = interp
            .eval_file(Path::new(OsStr::from_bytes(b"not/valid/utf8/\xff.rb")))
            .unwrap_err();
        assert_eq!("LoadError", err.name().as_ref());
        assert_eq!(
            b"ruby: file not found in virtual file system -- not/valid/utf8/\xFF.rb",
            err.message().as_ref()
        );
    }
}
