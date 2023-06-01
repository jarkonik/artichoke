// TODO: Remove unwraps

pub mod mruby;
mod trampoline;

use crate::extn::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct IO {
    fd: usize,
}

impl HeapAllocatedData for IO {
    const RUBY_TYPE: &'static str = "IO";
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::test::prelude::*;

    const SUBJECT: &str = "IO";
    const FUNCTIONAL_TEST: &[u8] = include_bytes!("io_test.rb");

    #[test]
    fn functional() {
        let mut interp = interpreter();
        interp
            .write_file(
                &Path::new("testfile"),
                std::borrow::Cow::Owned(
                    "This is line one\nThis is line two\nThis is line three\nAnd so on...\n"
                        .as_bytes()
                        .to_owned(),
                ),
            )
            .unwrap();
        interp
            .write_file(
                &Path::new("testfile2"),
                std::borrow::Cow::Owned(
                    "This is line one\nThis is line two\nThis is line three\nAnd so on...\n"
                        .as_bytes()
                        .to_owned(),
                ),
            )
            .unwrap();

        let result = interp.eval(FUNCTIONAL_TEST);
        unwrap_or_panic_with_backtrace(&mut interp, SUBJECT, result);
        let result = interp.eval(b"spec");
        unwrap_or_panic_with_backtrace(&mut interp, SUBJECT, result);
    }
}
