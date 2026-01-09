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
        let start = data[0];

        std::thread::scope(|scope| {
            let len = data.len().div_ceil(num_threads);

            let mut splits = (0..=num_threads)
                .map(|i| (i * len).min(data.len()))
                .collect::<Vec<usize>>();
            for x in &mut splits {
                if *x == 0 {
                    continue;
                }
                if *x >= data.len() {
                    continue;
                }
                // find first > or @ preceded by \n after x
                // TODO: Do this inside worker threads instead?
                // Or start threads as soon as each split is found?
                loop {
                    if let Some(pos) = memchr::memchr(start, &data[*x..]) {
                        if data[*x + pos - 1] == b'\n' {
                            *x += pos;
                            break;
                        }
                        *x += pos + 1;
                    } else {
                        *x = data.len();
                        break;
                    }
                }
            }

            let mut handles = vec![];
            for i in 0..num_threads {
                let mut worker_processor = processor.clone();
                let start = splits[i];
                let end = splits[i + 1];
                let handle = scope.spawn(move || {
                    let mut parser = FastxParser::<CONFIG>::from_slice(&data[start..end]);
                    let mut i = 0;
                    while let Some(_event) = parser.next() {
                        worker_processor.process_record(&parser)?;
                        i += 1;
                        if i == self.batch_size {
                            worker_processor.on_batch_complete()?;
                            i = 0;
                        }
                    }
                    if i > 0 {
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
