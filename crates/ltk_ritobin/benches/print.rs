use std::fs::read_to_string;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ltk_meta::Bin;
use ltk_ritobin::{Cst, Print};

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
        let mut group = c.benchmark_group("print");
        for sample in &samples {
            let size = sample.len();
            let cst = Cst::parse(sample);
            let (bin, _errs) = cst.build_bin(sample);

            group.throughput(Throughput::Bytes(size.try_into().unwrap()));
            group.bench_with_input(BenchmarkId::from_parameter(size), &bin, |b, bin| {
                b.iter(|| {
                    print(bin);
                })
            });
        }
    }
}

fn print(bin: &Bin) {
    let mut str = String::new();
    bin.print_to_writer(&mut str).unwrap();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
