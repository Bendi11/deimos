use std::sync::Arc;

use crate::server::Deimos;


pub struct ValheimService {
    state: Arc<Deimos>,
}

impl ValheimService {
    pub async fn task(self) -> ! {
        loop {

        }
    }
}
