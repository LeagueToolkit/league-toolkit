use std::{fs::read_to_string, hint::black_box};

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
        let mut group = c.benchmark_group("parse");
        for sample in &samples {
            let size = sample.len();
            group.throughput(Throughput::Bytes(size.try_into().unwrap()));
            group.bench_with_input(BenchmarkId::from_parameter(size), &sample, |b, sample| {
                b.iter(|| {
                    let _cst = black_box(Cst::parse(sample));
                })
            });
        }
    }

    {
        let mut group = c.benchmark_group("build_bin");
        for sample in &samples {
            let size = sample.len();

            let cst = Cst::parse(sample);

            group.throughput(Throughput::Bytes(size.try_into().unwrap()));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                &(cst, sample),
                |b, (cst, sample)| {
                    b.iter(|| {
                        let (_bin, _errs) = black_box(cst.build_bin(sample));
                    })
                },
            );
        }
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
