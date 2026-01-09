use paraseq::ProcessError;

use crate::{
    Config, FastxParser,
    input::{FromSlice, InputData, MmapInput},
};

pub struct ParallelHelicaseReader<'a, const CONFIG: Config> {
    mmap: MmapInput<'a>,
    batch_size: usize,
}

impl<'a, const CONFIG: Config> ParallelHelicaseReader<'a, CONFIG> {
    /// Batch size is the number of records between calls to `ParallelProcessor::on_batch_complete`.
    pub fn new(path: &std::path::Path, batch_size: usize) -> Self {
        Self {
            mmap: MmapInput::new(path).unwrap(),
            batch_size,
        }
    }
}

#[inline(always)]
fn find_fasta_boundary(data: &[u8], mut pos: usize) -> usize {
    while let Some(offset) = memchr::memchr(b'>', &data[pos..]) {
        let abs_pos = pos + offset;
        // Must be preceded by \n
        if abs_pos > 0 && data[abs_pos - 1] == b'\n' {
            return abs_pos;
        }
        pos = abs_pos + 1;
    }
    data.len()
}

#[inline(always)]
fn find_fastq_boundary(data: &[u8], mut pos: usize) -> usize {
    while let Some(offset) = memchr::memchr(b'\n', &data[pos..]) {
        let abs_pos = pos + offset;
        // \n must be followed by '@'
        if abs_pos + 1 >= data.len() || data[abs_pos + 1] != b'@' {
            pos = abs_pos + 1;
            continue;
        }
        // Check next line after '@'
        let remaining = &data[abs_pos..];
        // Search for the newline after the '@' (skipping \n and @)
        if let Some(newline_offset) = memchr::memchr(b'\n', &remaining[2..]) {
            let next_line_start = newline_offset + 3; // +2 offset + 1 for the \n itself
            // Continue search if we have a next line that starts with '@'
            // i.e. we have quality scores instead of valid record
            if next_line_start < remaining.len() && remaining[next_line_start] == b'@' {
                pos = abs_pos + 1;
                continue;
            }
        }
        return abs_pos + 1;
    }
    data.len()
}

impl<'b, const CONFIG: Config> paraseq::parallel::ParallelReader
    for ParallelHelicaseReader<'b, CONFIG>
{
    type Rf<'a> = &'a FastxParser<'a, CONFIG>;

    fn process_parallel<T>(self, processor: &mut T, num_threads: usize) -> paraseq::Result<()>
    where
        T: for<'a> paraseq::prelude::ParallelProcessor<Self::Rf<'a>>,
    {
        let data = self.mmap.data();
        assert!(data[0] == b'>' || data[0] == b'@');
        let is_fastq = data[0] == b'@';

        std::thread::scope(|scope| {
            let chunk_size = data.len().div_ceil(num_threads);

            let mut splits = (0..=num_threads)
                .map(|i| (i * chunk_size).min(data.len()))
                .collect::<Vec<usize>>();

            let len = splits.len();
            for split in &mut splits[1..len - 1] {
                *split = if is_fastq {
                    find_fastq_boundary(data, *split)
                } else {
                    find_fasta_boundary(data, *split)
                };
            }

            // We could try to split in same record, then multiple
            // splits will be data.len(), dedup to remove those
            splits.dedup();

            // Could end up with less threads than splits
            let actual_threads = splits.len() - 1;

            let mut handles = Vec::with_capacity(actual_threads);
            for i in 0..actual_threads {
                let mut worker_processor = processor.clone();
                let start = splits[i];
                let end = splits[i + 1];

                let handle = scope.spawn(move || {
                    let mut parser = FastxParser::<CONFIG>::from_slice(&data[start..end]);
                    let mut count = 0;

                    while let Some(_event) = parser.next() {
                        worker_processor.process_record(&parser)?;
                        count += 1;
                        if count == self.batch_size {
                            worker_processor.on_batch_complete()?;
                            count = 0;
                        }
                    }
                    if count > 0 {
                        worker_processor.on_batch_complete()?;
                    }
                    worker_processor.on_thread_complete()?;
                    Ok(())
                });
                handles.push(handle);
            }
            for handle in handles {
                match handle.join() {
                    Ok(Ok(())) => (),
                    Ok(Err(e)) => return Err(e),
                    Err(_) => return Err(ProcessError::JoinError),
                }
            }
            Ok(())
        })
    }

    fn process_parallel_paired<T>(
        self,
        _r2: Self,
        _processor: &mut T,
        _num_threads: usize,
    ) -> paraseq::Result<()>
    where
        T: for<'a> paraseq::prelude::PairedParallelProcessor<Self::Rf<'a>>,
    {
        todo!()
    }

    fn process_parallel_interleaved<T>(
        self,
        _processor: &mut T,
        _num_threads: usize,
    ) -> paraseq::Result<()>
    where
        T: for<'a> paraseq::prelude::PairedParallelProcessor<Self::Rf<'a>>,
    {
        todo!()
    }

    fn process_parallel_multi<T>(
        self,
        _rest: Vec<Self>,
        _processor: &mut T,
        _num_threads: usize,
    ) -> paraseq::Result<()>
    where
        T: for<'a> paraseq::prelude::MultiParallelProcessor<Self::Rf<'a>>,
        Self: Sized,
    {
        todo!()
    }

    fn process_parallel_multi_interleaved<T>(
        self,
        _arity: usize,
        _processor: &mut T,
        _num_threads: usize,
    ) -> paraseq::Result<()>
    where
        T: for<'a> paraseq::prelude::MultiParallelProcessor<Self::Rf<'a>>,
    {
        todo!()
    }
}
