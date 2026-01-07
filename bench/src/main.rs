mod measurement;
use measurement::{BaseTime, Measurement};

#[cfg(target_os = "linux")]
mod linux_perf;

use helicase::config::{advanced::*, *};
use helicase::input::*;
use helicase::*;

use needletail::{parse_fastx_file, parse_fastx_reader};
use paraseq::{Record, fastx};
use regex::bytes::RegexBuilder;

use std::env::args;
use std::fs::read;
use std::hint::black_box;
use std::path::Path;

const HEADER_ONLY: Config = ParserOptions::default().ignore_dna().config();
const DNA_STRING: Config = ParserOptions::default()
    .ignore_headers()
    .dna_string()
    .config();
const DNA_COLUMNAR: Config = ParserOptions::default()
    .ignore_headers()
    .dna_columnar()
    .config();
const DNA_PACKED: Config = ParserOptions::default()
    .ignore_headers()
    .dna_packed()
    .config();

struct Setup<'a, P: AsRef<Path>> {
    path: P,
    data: &'a [u8],
    size: u64,
    rep: u64,
    compressed: bool,
}

fn bench_config<const CONFIG: Config, P: AsRef<Path>, M: Measurement>(label: &str, s: &Setup<P>) {
    let mut m = M::new();

    for _ in 0..s.rep {
        let parser = FastxParser::<CONFIG>::from_file(&s.path).expect("Cannot open file");
        parser.for_each(|ev| {
            black_box(&ev);
        });
    }

    if !s.compressed {
        #[cfg(not(feature="slices_only"))]
        {
        m.start();
        for _ in 0..s.rep {
            let parser = FastxParser::<CONFIG>::from_file_mmap(&s.path).unwrap();
            parser.for_each(|ev| {
                black_box(ev);
            });
        }
        let lab = format!("{label} (mmap)");
        m.show(&lab, s.size, s.rep);
        }
        m.start();
        for _ in 0..s.rep {
            let parser = FastxParser::<CONFIG>::from_slice(s.data);
            parser.for_each(|ev| {
                black_box(ev);
            });
        }
        let lab = format!("{label} (slice)");
        m.show(&lab, s.size, s.rep);
    } else {

        #[cfg(not(feature="slices_only"))]
        {
        m.start();
        for _ in 0..s.rep {
            let parser = FastxParser::<CONFIG>::from_reader(s.data);
            parser.for_each(|ev| {
                black_box(ev);
            });
        }
        let lab = format!("{label} (reader)");
        m.show(&lab, s.size, s.rep);
        }
    }
}

fn measurment_variant<M: Measurement, P: AsRef<Path>>(s: Setup<P>) {
    let mut m = M::new();

    #[cfg(feature = "regex")]
    if !s.compressed {
        let match_dna = RegexBuilder::new(r"(>[^\n]*\n)").build().unwrap();
        m.start();
        for _ in 0..s.rep {
            match_dna.find_iter(s.data).for_each(|m| {
                black_box(m);
            });
        }
        m.show("Regex header (slice)", s.size, s.rep);
    }

    #[cfg(feature ="needletail")]
    {
    m.start();
    for _ in 0..s.rep {
        let mut reader = parse_fastx_file(&s.path).expect("invalid file");
        while let Some(r) = reader.next() {
            let record = r.expect("invalid record");
            let clean_seq = record.seq();
            black_box(clean_seq);
        }
    }
    m.show("Needletail (file)", s.size, s.rep);
    m.start();
    for _ in 0..s.rep {
        let mut reader = parse_fastx_reader(s.data).expect("invalid reader");
        while let Some(r) = reader.next() {
            let record = r.expect("invalid record");
            let clean_seq = record.seq();
            black_box(clean_seq);
        }
    }
    m.show("Needletail (reader)", s.size, s.rep);

    m.start();
    for _ in 0..s.rep {
        let mut reader = parse_fastx_file(&s.path).expect("invalid file");
        let mut base = 0usize;
        while let Some(r) = reader.next() {
            let record = r.expect("invalid record");
            base += record.num_bases();
        }
        black_box(base);
    }
    m.show("Needletail (dna len)", s.size, s.rep);
    }
    #[cfg(feature ="paraseq")]
    if !s.compressed {
        m.start();
        for _ in 0..s.rep {
            // let mut reader = fastx::Reader::from_path(&path).expect("invalid file"); // crashes on human genome
            let mut reader =
                fastx::Reader::from_path_with_batch_size(&s.path, 1).expect("invalid file");
            let mut record_set = reader.new_record_set();
            while record_set.fill(&mut reader).unwrap() {
                for r in record_set.iter() {
                    let record = r.expect("invalid record");
                    let clean_seq = record.seq();
                    black_box(clean_seq);
                }
            }
        }
        m.show("Paraseq (file)", s.size, s.rep);
        m.start();
        for _ in 0..s.rep {
            // let mut reader = fastx::Reader::new(data).expect("invalid reader"); // crashes on human genome
            let mut reader = fastx::Reader::new_with_batch_size(s.data, 1).expect("invalid reader");
            let mut record_set = reader.new_record_set();
            while record_set.fill(&mut reader).unwrap() {
                for r in record_set.iter() {
                    let record = r.expect("invalid record");
                    let clean_seq = record.seq();
                    black_box(clean_seq);
                }
            }
        }
        m.show("Paraseq (reader)", s.size, s.rep);
    }

    println!("---");
    bench_config::<HEADER_ONLY, _, M>("Header only", &s);

    #[cfg(not(feature ="header_only"))]
    {
    bench_config::<DNA_STRING, _, M>("DNA string", &s);
    bench_config::<DNA_PACKED, _, M>("DNA packed", &s);
    bench_config::<DNA_COLUMNAR, _, M>("DNA columnar", &s);

    if !s.compressed {
        m.start();
        for _ in 0..s.rep {
            let mut parser = FastaParser::<
                { COMPUTE_DNA_LEN | SPLIT_NON_ACTG | MERGE_DNA_CHUNKS | MERGE_RECORDS },
                _,
            >::from_slice(s.data);
            parser.next();
            //black_box(parser.get_dna_len());

        }
        m.show("DNA len (slice)", s.size, s.rep);
    }
    }
}

fn main() {
    let path = args().nth(1).expect("No input file given");
    let content = read(&path).expect("Cannot open file");
    let data = content.as_slice();
    let size = data.len() as u64;
    let mut input_file = FileInput::new(&path).expect("Cannot open file");
    let compressed = input_file.is_compressed().unwrap();
    let rep = 3;

    let s = Setup {
        path: &path,
        data,
        size,
        compressed,
        rep,
    };
    if cfg!(target_os = "linux") {
        use linux_perf::PerfMeasurement;
        measurment_variant::<PerfMeasurement, _>(s);
    } else {
        measurment_variant::<BaseTime, _>(s);
    }
}
