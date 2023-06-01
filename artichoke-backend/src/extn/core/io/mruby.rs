use std::{ffi::CStr, os::unix::raw::off_t, path::Path};

use crate::extn::{core::io::trampoline, prelude::*};

use super::IO;

const IO_CSTR: &CStr = qed::const_cstr_from_str!("IO\0");
static IO_RUBY_SOURCE: &[u8] = include_bytes!("io.rb");

pub fn init(interp: &mut Artichoke) -> InitializeResult<()> {
    if interp.is_class_defined::<IO>() {
        return Ok(());
    }

    let spec = class::Spec::new("IO", IO_CSTR, None, Some(def::box_unbox_free::<IO>))?;
    class::Builder::for_spec(interp, &spec)
        .add_method("initialize", io_initialize, sys::mrb_args_req(1) | sys::mrb_args_opt(2))?
        .add_self_method("binread", io_binread, sys::mrb_args_req(1) | sys::mrb_args_opt(2))?
        .add_self_method("write", io_write, sys::mrb_args_req(2) | sys::mrb_args_opt(2))?
        .define()?;
    interp.def_class::<IO>(spec)?;
    interp.eval(IO_RUBY_SOURCE)?;

    Ok(())
}

unsafe extern "C" fn io_initialize(mrb: *mut sys::mrb_state, slf: sys::mrb_value) -> sys::mrb_value {
    let (fd, mode, a) = mrb_get_args!(mrb, required = 1, optional = 2);
    unwrap_interpreter!(mrb, to => guard);
    let value = Value::from(slf);
    let result = trampoline::initialize(&mut guard, value, fd.into());
    match result {
        Ok(value) => value.inner(),
        Err(exception) => error::raise(guard, exception),
    }
}

unsafe extern "C" fn io_binread(mrb: *mut sys::mrb_state, slf: sys::mrb_value) -> sys::mrb_value {
    let (name, length, offset) = mrb_get_args!(mrb, required = 1, optional = 2);
    unwrap_interpreter!(mrb, to => guard);
    let name: String = guard.try_convert_mut(Value::from(name)).unwrap();

    let mut string = guard.read_file(name).unwrap().to_vec();

    // TODO: Dont copy
    if let Some(offset) = offset {
        let offset: usize = guard.try_convert(Value::from(offset)).unwrap();
        string = (&string[offset..]).to_owned();
    }
    // TODO: Dont copy
    if let Some(length) = length {
        let length: usize = guard.try_convert(Value::from(length)).unwrap();
        string = (&string[0..length]).to_owned();
    }

    let message = guard.try_convert_mut(string).ok().unwrap();
    message.inner()
}

unsafe extern "C" fn io_write(mrb: *mut sys::mrb_state, slf: sys::mrb_value) -> sys::mrb_value {
    let (name, string, offset, opt) = mrb_get_args!(mrb, required = 2, optional = 2);
    unwrap_interpreter!(mrb, to => guard);

    let name: String = guard.try_convert_mut(Value::from(name)).unwrap();
    let string: String = guard.try_convert_mut(Value::from(string)).unwrap();

    match offset {
        Some(offset) => {
            let offset: usize = guard.try_convert(Value::from(offset)).unwrap();
            let mut contents = guard.read_file(name.clone()).unwrap().to_vec();
            contents[offset..][..string.len()].copy_from_slice(string.as_bytes());
            guard
                .write_file(Path::new(&name), std::borrow::Cow::Owned(contents))
                .unwrap();
        }
        None => {
            guard
                .write_file(Path::new(&name), std::borrow::Cow::Owned(string.as_bytes().to_vec()))
                .unwrap();
        }
    }

    guard.try_convert(string.len()).unwrap().inner()
}
