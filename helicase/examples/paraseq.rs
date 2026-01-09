use helicase::paraseq_reader::ParallelHelicaseReader;
use helicase::*;
use paraseq::Record;
use paraseq::prelude::{ParallelProcessor, ParallelReader};
use std::sync::atomic::AtomicUsize;

// set the options of the parser (at compile-time)
const CONFIG: Config = ParserOptions::default().config();

fn main() {
    let path = std::env::args().nth(1).expect("No input file given");

    // create a parser with the desired options
    let reader = ParallelHelicaseReader::<CONFIG>::new(std::path::Path::new(&path), 16);

    let count = AtomicUsize::new(0);
    let count_ref = &count;
    // Create a parallel processor from a closure that is called for every record.
    let mut processor = RecordProcessor(move |_record: &FastxParser<CONFIG>| {
        // NOTE: In actual usage you shouldn't take a global lock for each read.
        // Instead, you should implement `ParallelProcessor` manually.
        count_ref.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    });

    reader.process_parallel(&mut processor, 6).unwrap();
    let count = count.into_inner();
    eprintln!("Total records: {}", count);
}

/// Little wrapper that will be added to paraseq
#[derive(Clone)]
pub struct RecordProcessor<F>(pub F);

impl<Rf: Record, F> ParallelProcessor<Rf> for RecordProcessor<F>
where
    F: for<'a> FnMut(Rf) -> paraseq::Result<()> + Send + Clone,
{
    fn process_record(&mut self, record: Rf) -> paraseq::Result<()> {
        (self.0)(record)
    }
}
