//! Compares the two typecheckers' phases against the same parsed input: `typecheck::walk`
//! (`Cst::build_bin`, CST -> `Bin` in one pass) vs. the new `ast` engine's two halves
//! (`Cst::build_ast`, CST -> `Ast`; and `Ast::to_bin`, `Ast` -> `Bin`) - see the crate's design
//! notes for why `ast` is a second, independent implementation rather than a shared pipeline.
//!
//! Parsing itself is excluded (see `benches/parse.rs`): the `Cst` (and, for the `ast_to_bin`
//! group, the `Ast`) is built once outside every timed closure, so each group measures only the
//! one phase it names.

use std::fs::read_to_string;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ltk_ritobin::Cst;

fn criterion_benchmark(c: &mut Criterion) {
    let dir = env!("CARGO_MANIFEST_DIR");
    let samples = [
        read_to_string(format!("{dir}/samples/aatrox.rito")).unwrap(),
        read_to_string(format!("{dir}/samples/azirultsoldier.rito")).unwrap(),
        read_to_string(format!("{dir}/samples/big.rito")).unwrap(),
        read_to_string(format!("{dir}/samples/skin38.rito")).unwrap(),
        read_to_string(format!("{dir}/samples/test.rito")).unwrap(),
        read_to_string(format!("{dir}/samples/zaahen.rito")).unwrap(),
    ];

    {
        let mut group = c.benchmark_group("cst_to_bin");
        for sample in &samples {
            let size = sample.len();
            let cst = Cst::parse(sample);

            group.throughput(Throughput::Bytes(size.try_into().unwrap()));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                &(cst, sample),
                |b, (cst, sample)| {
                    b.iter(|| {
                        let _partial = std::hint::black_box(cst.build_bin(sample));
                    })
                },
            );
        }
    }

    {
        let mut group = c.benchmark_group("cst_to_ast");
        for sample in &samples {
            let size = sample.len();
            let cst = Cst::parse(sample);

            group.throughput(Throughput::Bytes(size.try_into().unwrap()));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                &(cst, sample),
                |b, (cst, sample)| {
                    b.iter(|| {
                        let _ast = std::hint::black_box(cst.build_ast(sample));
                    })
                },
            );
        }
    }

    {
        let mut group = c.benchmark_group("ast_to_bin");
        for sample in &samples {
            let size = sample.len();
            let cst = Cst::parse(sample);
            let ast = cst.build_ast(sample);

            group.throughput(Throughput::Bytes(size.try_into().unwrap()));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                &(ast, sample),
                |b, (ast, sample)| {
                    b.iter(|| {
                        let _bin = std::hint::black_box(ast.to_bin(sample));
                    })
                },
            );
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
