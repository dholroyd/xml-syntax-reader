//! Instruction-count regression benchmark, run under Valgrind via gungraun.
//!
//! Requires `gungraun-runner` matching the `gungraun` dev-dependency version:
//! `cargo install gungraun-runner --version 0.20.0`, then `cargo bench --bench gungraun`.

mod combined_workload;

use std::hint::black_box;

use gungraun::prelude::*;
use gungraun::Callgrind;
use xml_syntax_reader::{parse_read_with_capacity, Reader, Visitor};

struct NullVisitor;

impl Visitor for NullVisitor {
    type Error = std::convert::Infallible;
}

fn setup_combined() -> Vec<u8> {
    combined_workload::gen_combined(256 * 1024)
}

// Parses the document once from a single buffer, then again streamed
// through an 8 KiB buffer to cover buffer shifts and resumption.
#[library_benchmark]
#[bench::combined(setup = setup_combined)]
fn parse(doc: Vec<u8>) {
    let mut reader = Reader::new();
    reader
        .parse(black_box(&doc), 0, true, &mut NullVisitor)
        .unwrap();
    parse_read_with_capacity(black_box(&doc[..]), &mut NullVisitor, 8192).unwrap();
}

library_benchmark_group!(name = parser, benchmarks = parse);

// Callgrind sizes its simulated caches from the host CPU, which would make
// the cache-derived metrics (including estimated cycles) vary with whatever
// hardware the CI runner lands on. Fix the geometry so results are comparable.
main!(
    config = LibraryBenchmarkConfig::default().tool(Callgrind::with_args([
        "--I1=32768,8,64",
        "--D1=32768,8,64",
        "--LL=8388608,16,64",
    ])),
    library_benchmark_groups = parser
);
