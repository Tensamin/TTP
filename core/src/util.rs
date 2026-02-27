use rand::RngCore;
use rand::rngs::OsRng;

pub fn rand_u32() -> u32 {
    let mut rng = OsRng;
    rng.next_u32()
}
