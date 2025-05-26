use prometheus::{IntCounter, register_int_counter, Encoder, TextEncoder, gather};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref METEORA_SWAP_SUCCESS: IntCounter = register_int_counter!(
        "meteora_swap_success_total", "Успешные свапы Meteora"
    ).unwrap();

    pub static ref METEORA_SWAP_FAILURE: IntCounter = register_int_counter!(
        "meteora_swap_failure_total", "Неудачные свапы Meteora"
    ).unwrap();

    pub static ref METEORA_POOL_DETECTED: IntCounter = register_int_counter!(
        "meteora_pools_detected_total", "Обнаруженные пулы Meteora"
    ).unwrap();
}

pub fn encode_metrics() -> Vec<u8> {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&gather(), &mut buffer).unwrap();
    buffer
}
