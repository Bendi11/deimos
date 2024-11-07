use fltk::{group::Group, prelude::*};

use super::DeimosStateHandle;



pub fn authorization(state: DeimosStateHandle) -> Group {
    let mut top = Group::default_fill();
    top.hide();

    top.end();
    top
}
