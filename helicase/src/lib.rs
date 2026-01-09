mod carrying_add;
pub mod config;
pub mod dna_format;
pub mod input;
pub mod lexer;
#[cfg(feature = "paraseq")]
pub mod paraseq_reader;
pub mod parser;

pub use config::{Config, ParserOptions};
pub use parser::{Event, FastaParser, FastqParser, FastxParser, Parser};

#[cfg(target_feature = "avx2")]
pub(crate) mod simd {
    mod avx2;
    pub use avx2::*;
}
#[cfg(target_feature = "neon")]
pub(crate) mod simd {
    mod neon;
    pub use neon::*;
}
#[cfg(not(any(target_feature = "avx2", target_feature = "neon")))]
#[deprecated(
    note = "This parser uses AVX2 or NEON SIMD instructions. Compile using `-C target-cpu=native` to get the expected performance."
)]
pub(crate) mod simd {
    mod fallback;
    pub use fallback::*;
}
