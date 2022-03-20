use lazy_static::lazy_static;
use std::collections::HashMap;

fn translate_addr_null(addr: u8) -> u8 {
    addr
}

lazy_static! {
    pub static ref ADDRESS_MAPPER: HashMap<u8, fn(u8) -> u8> = {
        let mut m = HashMap::from([
            (0 as u8, translate_addr_null as fn(u8) -> u8),
        ]);
        m
    };
}