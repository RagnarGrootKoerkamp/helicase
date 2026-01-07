use std::time::Instant;

pub trait Measurement {
    fn start(&mut self);
    fn show(&mut self, label: &str, size: u64, rep: u64);
    fn new() -> Self;
}

pub struct BaseTime(Option<Instant>);

impl Measurement for BaseTime {
    fn new() -> Self {
        Self(None)
    }
    fn start(&mut self) {
        self.0 = Some(Instant::now());
    }
    fn show(&mut self, label: &str, size: u64, rep: u64) {
        let val = self.0.unwrap().elapsed().as_secs_f64();
        println!("{label}:\t {:5.2} GB/s", (size * rep) as f64 / 1e9 / val);
    }
}
