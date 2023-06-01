use crate::extn::prelude::*;

pub fn initialize(interp: &mut Artichoke, into: Value, fd: Value) -> Result<Value, Error> {
    let io = super::IO { fd: 0 };
    super::IO::box_into_value(io, into, interp)
}
