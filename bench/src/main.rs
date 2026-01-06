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
use std::time::Instant;

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

trait Measurement {
    fn start(&mut self);
    fn show(&self, label: &str, size: u64, rep: u64);
    fn new() -> Self;
}

struct BaseTime(Option<Instant>);


impl Measurement for BaseTime {
    fn new() -> Self {
        Self(None)
    }
    fn start(&mut self){
        self.0 = Some(Instant::now());
    }
    fn show(&self, label: &str, size: u64, rep: u64) {
        let val = self.0.unwrap().elapsed().as_secs_f64();
        println!(
            "{label}:\t {:5.2} GB/s",
            (size * rep) as f64 / 1e9 / val
        );
    }
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
        m.start();
        for _ in 0..s.rep {
            let parser = FastxParser::<CONFIG>::from_file_mmap(&s.path).unwrap();
            parser.for_each(|ev| {
                black_box(ev);
            });
        }
        let lab = format!("{label} (mmap)");
        m.show(&lab, s.size, s.rep);
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
        m.start();
        let now = Instant::now();
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

    let mut m = BaseTime::new();

    if !compressed {
        let match_dna = RegexBuilder::new(r"(>[^\n]*\n)").build().unwrap();
        m.start();
        for _ in 0..rep {
            match_dna.find_iter(data).for_each(|m| {
                black_box(m);
            });
        }
        m.show("Regex header (slice)", size, rep);
    }

    m.start();
    for _ in 0..rep {
        let mut reader = parse_fastx_file(&path).expect("invalid file");
        while let Some(r) = reader.next() {
            let record = r.expect("invalid record");
            let clean_seq = record.seq();
            black_box(clean_seq);
        }
    }
    m.show("Needletail (file)", size, rep);
    m.start();
    for _ in 0..rep {
        let mut reader = parse_fastx_reader(data).expect("invalid reader");
        while let Some(r) = reader.next() {
            let record = r.expect("invalid record");
            let clean_seq = record.seq();
            black_box(clean_seq);
        }
    }
    m.show("Needletail (reader)", size, rep);

    if !compressed {
        m.start();
        for _ in 0..rep {
            // let mut reader = fastx::Reader::from_path(&path).expect("invalid file"); // crashes on human genome
            let mut reader =
                fastx::Reader::from_path_with_batch_size(&path, 1).expect("invalid file");
            let mut record_set = reader.new_record_set();
            while record_set.fill(&mut reader).unwrap() {
                for r in record_set.iter() {
                    let record = r.expect("invalid record");
                    let clean_seq = record.seq();
                    black_box(clean_seq);
                }
            }
        }
        m.show("Paraseq (file)", size, rep);
        m.start();
        for _ in 0..rep {
            // let mut reader = fastx::Reader::new(data).expect("invalid reader"); // crashes on human genome
            let mut reader = fastx::Reader::new_with_batch_size(data, 1).expect("invalid reader");
            let mut record_set = reader.new_record_set();
            while record_set.fill(&mut reader).unwrap() {
                for r in record_set.iter() {
                    let record = r.expect("invalid record");
                    let clean_seq = record.seq();
                    black_box(clean_seq);
                }
            }
        }
        m.show("Paraseq (reader)", size, rep);
    }

    println!("---");

    bench_config::<HEADER_ONLY, _, BaseTime>("Header only", &s);
    bench_config::<DNA_STRING, _, BaseTime>("DNA string", &s);
    bench_config::<DNA_PACKED, _, BaseTime>("DNA packed", &s);
    bench_config::<DNA_COLUMNAR, _, BaseTime>("DNA columnar", &s);

    if !compressed {
        m.start();
        let now = Instant::now();
        for _ in 0..rep {
            let mut parser = FastaParser::<
                { COMPUTE_DNA_LEN | SPLIT_NON_ACTG | MERGE_DNA_CHUNKS | MERGE_RECORDS },
                _,
            >::from_slice(data);
            parser.next();
            black_box(parser.get_dna_len());
        }
        m.show("DNA len (slice)", size, rep);
    }
}
