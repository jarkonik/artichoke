use std::ffi::CStr;

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
    unwrap_interpreter!(mrb, to => guard);
    let contents = guard.read_file("testfile").unwrap().to_vec();

    let message = guard.try_convert_mut(contents).ok().unwrap();
    message.inner()
}
