//! Benchmarks for the incremental-parsing and arena-layout claims in
//! `docs/design/ltk-ritobin-span-model.md` (its section 11 holds the readings).
//!
//! Groups:
//!
//! - `splice_tail_shift` - the constant shift of section 5.2 step 5.
//! - `node_span` - a stored `Node.span` against the derived `span(cst)` of 4.2.
//! - `entry_reparse` - one item of the top-level entries map, the splice unit.
//! - `children_layout` - close-order against pre-order children, section 6.3 item 3.
//! - `wrapper_nodes` - virtual wrapper ids, section 6.4.
//!
//! `Cst` keeps its arena private, so this bench rebuilds an equivalent one
//! through the public API. A pre-order walk gives node order and token order.
//! A post-order walk gives the child order that `build_tree` produces at
//! `Close`. Both reconstructions happen in setup and are not timed.

use std::fs::read_to_string;
use std::hint::black_box;
use std::time::Duration;

use criterion::{
    criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode, Throughput,
};
use ltk_ritobin::cst::{Child, Kind, NodeId};
use ltk_ritobin::parse::{Span, Token};
use ltk_ritobin::Cst;

const FILES: &[&str] = &[
    "aatrox.rito",
    "azirultsoldier.rito",
    "big.rito",
    "zaahen.rito",
    "skin38.rito",
];

fn load(name: &str) -> String {
    let dir = env!("CARGO_MANIFEST_DIR");
    read_to_string(format!("{dir}/samples/{name}")).unwrap()
}

/// The sample corpus, plus any files named in `LTK_RITOBIN_BENCH_EXTRA`.
///
/// The variable holds absolute paths separated by `;`, so a file outside the
/// repository can join a run without a code change. Convert a `.bin` first with
/// `cargo run --release --example bin_to_rito`.
fn corpus() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> =
        FILES.iter().map(|f| ((*f).to_string(), load(f))).collect();
    for path in std::env::var("LTK_RITOBIN_BENCH_EXTRA")
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let name = std::path::Path::new(path)
            .file_name()
            .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
        let text = read_to_string(path)
            .unwrap_or_else(|e| panic!("LTK_RITOBIN_BENCH_EXTRA: cannot read {path}: {e}"));
        out.push((name, text));
    }
    out
}

/// A child reference in the rebuilt arena. Indices are this arena's own.
#[derive(Clone, Copy)]
enum Ref {
    Token(u32),
    Tree(u32),
}

/// The target model's node row: a `TokenRange` sits where the `Span` sat, so
/// the row keeps today's size and the walks below compare like with like.
#[derive(Clone, Copy)]
struct Row {
    tokens: (u32, u32),
    kind: Kind,
    children: (u32, u32),
    errors: (u32, u32),
}

/// Today's node row. It must stay the same size as `Row`, or every reading
/// below measures the row width instead of the thing under test.
#[derive(Clone, Copy)]
struct TodayRow {
    span: Span,
    kind: Kind,
    children: (u32, u32),
    #[allow(dead_code, reason = "carried so the row keeps `Row`'s width")]
    errors: (u32, u32),
}

struct Arena {
    rows: Vec<Row>,
    /// The same tree in today's layout, for the A side of every comparison.
    today: Vec<TodayRow>,
    tokens: Vec<Token>,
    /// Children in close order, which is the layout `build_tree` produces.
    close: Vec<Ref>,
    /// Children in pre-order, the CSR layout of 6.3 item 3.
    pre: Vec<Ref>,
    /// `pre` boundaries, `len + 1` entries.
    pre_start: Vec<u32>,
}

impl Arena {
    fn build(cst: &Cst) -> Self {
        assert_eq!(
            std::mem::size_of::<Row>(),
            std::mem::size_of::<TodayRow>(),
            "the two layouts must stay the same width"
        );
        let mut a = Arena {
            rows: Vec::new(),
            today: Vec::new(),
            tokens: Vec::new(),
            close: Vec::new(),
            pre: Vec::new(),
            pre_start: Vec::new(),
        };
        // The root is the only node without an id, so walk its children by hand.
        let root = cst.root();
        let idx = a.push_row(root.kind, root.span);
        let refs = a.walk_children(cst, root, idx);
        a.finish_node(idx, refs);

        // Second pass: the same slices in pre-order.
        a.pre_start = Vec::with_capacity(a.rows.len() + 1);
        let mut acc = 0u32;
        for i in 0..a.rows.len() {
            a.pre_start.push(acc);
            let (s, n) = a.rows[i].children;
            for k in 0..n {
                a.pre.push(a.close[(s + k) as usize]);
            }
            acc += n;
        }
        a.pre_start.push(acc);
        a
    }

    fn push_row(&mut self, kind: Kind, span: Span) -> u32 {
        let idx = self.rows.len() as u32;
        self.rows.push(Row {
            tokens: (0, 0),
            kind,
            children: (0, 0),
            errors: (0, 0),
        });
        self.today.push(TodayRow {
            span,
            kind,
            children: (0, 0),
            errors: (0, 0),
        });
        idx
    }

    fn walk_children(&mut self, cst: &Cst, node: &ltk_ritobin::Node, idx: u32) -> Vec<Ref> {
        let (mut lo, mut hi) = (u32::MAX, 0u32);
        let mut refs = Vec::new();
        for child in node.children.get(cst) {
            match child {
                Child::Token(t) => {
                    let ti = self.tokens.len() as u32;
                    self.tokens.push(*cst.token(*t).unwrap());
                    refs.push(Ref::Token(ti));
                    lo = lo.min(ti);
                    hi = hi.max(ti + 1);
                }
                Child::Tree(n) => {
                    let ci = self.visit(cst, *n);
                    let (cs, cn) = self.rows[ci as usize].tokens;
                    if cn != 0 {
                        lo = lo.min(cs);
                        hi = hi.max(cs + cn);
                    }
                    refs.push(Ref::Tree(ci));
                }
            }
        }
        self.rows[idx as usize].tokens = if lo == u32::MAX {
            (0, 0)
        } else {
            (lo, hi - lo)
        };
        refs
    }

    fn visit(&mut self, cst: &Cst, id: NodeId) -> u32 {
        let node = cst.node(id).unwrap();
        let idx = self.push_row(node.kind, node.span);
        let refs = self.walk_children(cst, node, idx);
        self.finish_node(idx, refs);
        idx
    }

    /// Append this node's slice at close time, exactly as `build_tree` does.
    fn finish_node(&mut self, idx: u32, refs: Vec<Ref>) {
        let start = self.close.len() as u32;
        self.close.extend(refs);
        let range = (start, self.close.len() as u32 - start);
        self.rows[idx as usize].children = range;
        self.today[idx as usize].children = range;
    }
}

/// Deterministic shuffle, so the random-access reading does not need a dependency.
fn shuffled(n: usize) -> Vec<u32> {
    let mut order: Vec<u32> = (0..n as u32).collect();
    let mut st = 0x9E37_79B9_7F4A_7C15u64;
    for i in (1..order.len()).rev() {
        st ^= st << 13;
        st ^= st >> 7;
        st ^= st << 17;
        order.swap(i, (st % (i as u64 + 1)) as usize);
    }
    order
}

/// The spans of the items in the top-level `entries` map, which is the unit a
/// splice replaces. The root itself is not that unit: one root entry covers
/// almost the whole file (section 11).
fn entry_items(cst: &Cst, text: &str) -> Vec<Span> {
    let root = cst.root();
    let mut widest: Option<(NodeId, u32)> = None;
    for child in root.children.get(cst) {
        if let Child::Tree(id) = child {
            let n = cst.node(*id).unwrap();
            let w = n.span.end - n.span.start;
            if widest.is_none_or(|(_, bw)| w > bw) {
                widest = Some((*id, w));
            }
        }
    }
    let Some((entry, _)) = widest else {
        return vec![];
    };

    // first Block under that entry
    let mut queue = std::collections::VecDeque::from([entry]);
    let mut block = None;
    while let Some(id) = queue.pop_front() {
        let n = cst.node(id).unwrap();
        if n.kind == Kind::Block {
            block = Some(n);
            break;
        }
        for child in n.children.get(cst) {
            if let Child::Tree(c) = child {
                queue.push_back(*c);
            }
        }
    }
    let Some(block) = block else { return vec![] };

    let mut items: Vec<Span> = block
        .children
        .get(cst)
        .iter()
        .filter_map(|c| match c {
            Child::Tree(id) => Some(cst.node(*id).unwrap().span),
            Child::Token(_) => None,
        })
        .filter(|s| s.end > s.start && (s.end as usize) <= text.len())
        .collect();
    items.sort_by_key(|s| s.end - s.start);
    items
}

fn benches(c: &mut Criterion) {
    let samples = corpus();
    let arenas: Vec<(&str, Arena, Cst)> = samples
        .iter()
        .map(|(name, text)| {
            let cst = Cst::parse(text);
            let arena = Arena::build(&cst);
            (name.as_str(), arena, cst)
        })
        .collect();

    // ---- section 5.2 step 5: the constant tail shift --------------------
    {
        let mut g = c.benchmark_group("splice_tail_shift");
        g.sample_size(50);
        for (name, arena, _) in &arenas {
            let n = arena.tokens.len();
            g.throughput(Throughput::Elements(n as u64));
            let mut tokens = arena.tokens.clone();
            let mut rows = arena.rows.clone();
            let mut close = arena.close.clone();
            g.bench_function(BenchmarkId::from_parameter(name), |b| {
                b.iter(|| {
                    for t in tokens.iter_mut() {
                        t.span.start = t.span.start.wrapping_add(1);
                        t.span.end = t.span.end.wrapping_add(1);
                    }
                    for r in rows.iter_mut() {
                        r.tokens.0 = r.tokens.0.wrapping_add(1);
                        r.children.0 = r.children.0.wrapping_add(1);
                        r.errors.0 = r.errors.0.wrapping_add(1);
                    }
                    for ch in close.iter_mut() {
                        *ch = match *ch {
                            Ref::Token(t) => Ref::Token(t.wrapping_add(1)),
                            Ref::Tree(t) => Ref::Tree(t.wrapping_add(1)),
                        };
                    }
                    black_box((&tokens, &rows, &close));
                })
            });
        }
    }

    // ---- section 4.2: stored span against derived span ------------------
    {
        let mut g = c.benchmark_group("node_span");
        g.sample_size(50);
        for (name, arena, _) in &arenas {
            let n = arena.rows.len();
            let order = shuffled(n);
            g.throughput(Throughput::Elements(n as u64));

            g.bench_function(BenchmarkId::new("stored_seq", name), |b| {
                b.iter(|| {
                    let mut acc = 0u64;
                    for r in arena.today.iter() {
                        acc += (r.span.end - r.span.start) as u64;
                    }
                    black_box(acc)
                })
            });
            g.bench_function(BenchmarkId::new("derived_seq", name), |b| {
                b.iter(|| {
                    let mut acc = 0u64;
                    for r in arena.rows.iter() {
                        acc += derive(&arena.tokens, r.tokens) as u64;
                    }
                    black_box(acc)
                })
            });
            g.bench_function(BenchmarkId::new("stored_rand", name), |b| {
                b.iter(|| {
                    let mut acc = 0u64;
                    for &i in order.iter() {
                        let s = arena.today[i as usize].span;
                        acc += (s.end - s.start) as u64;
                    }
                    black_box(acc)
                })
            });
            g.bench_function(BenchmarkId::new("derived_rand", name), |b| {
                b.iter(|| {
                    let mut acc = 0u64;
                    for &i in order.iter() {
                        acc += derive(&arena.tokens, arena.rows[i as usize].tokens) as u64;
                    }
                    black_box(acc)
                })
            });
        }
    }

    // ---- the splice unit: one item of the entries map --------------------
    {
        let mut g = c.benchmark_group("entry_reparse");
        g.sampling_mode(SamplingMode::Flat);
        g.sample_size(20);
        g.measurement_time(Duration::from_secs(6));
        for ((name, text), (_, _, cst)) in samples.iter().zip(&arenas) {
            let items = entry_items(cst, text);
            if items.is_empty() {
                continue;
            }
            // `items` is sorted by width, so this is the p95 item by size.
            let p95 = items[(items.len() * 95 / 100).min(items.len() - 1)];
            let frag = &text[p95.start as usize..p95.end as usize];

            g.throughput(Throughput::Bytes(frag.len() as u64));
            g.bench_function(BenchmarkId::new("p95_item", name), |b| {
                b.iter(|| black_box(Cst::parse(frag)))
            });
            g.throughput(Throughput::Bytes(text.len() as u64));
            g.bench_function(BenchmarkId::new("whole_file", name), |b| {
                b.iter(|| black_box(Cst::parse(text)))
            });
        }
    }

    // ---- section 6.3 item 3: close order against pre-order ---------------
    {
        let mut g = c.benchmark_group("children_layout");
        g.sample_size(50);
        for (name, arena, _) in &arenas {
            g.throughput(Throughput::Elements(arena.close.len() as u64));

            g.bench_function(BenchmarkId::new("walk_close_order", name), |b| {
                b.iter(|| {
                    let mut acc = 0u64;
                    for r in arena.rows.iter() {
                        let (s, n) = r.children;
                        for ch in &arena.close[s as usize..(s + n) as usize] {
                            acc += id_of(*ch) as u64;
                        }
                    }
                    black_box(acc)
                })
            });
            g.bench_function(BenchmarkId::new("walk_pre_order", name), |b| {
                b.iter(|| {
                    let mut acc = 0u64;
                    for i in 0..arena.rows.len() {
                        let s = arena.pre_start[i] as usize;
                        let e = arena.pre_start[i + 1] as usize;
                        for ch in &arena.pre[s..e] {
                            acc += id_of(*ch) as u64;
                        }
                    }
                    black_box(acc)
                })
            });
            // The price of reaching the pre-order layout by a separate pass.
            g.bench_function(BenchmarkId::new("permute_to_pre_order", name), |b| {
                let mut out: Vec<Ref> = Vec::with_capacity(arena.close.len());
                b.iter(|| {
                    out.clear();
                    for r in arena.rows.iter() {
                        let (s, n) = r.children;
                        out.extend_from_slice(&arena.close[s as usize..(s + n) as usize]);
                    }
                    black_box(&out);
                })
            });
        }
    }

    // ---- section 6.4: virtual wrapper nodes ------------------------------
    {
        let mut g = c.benchmark_group("wrapper_nodes");
        g.sample_size(50);
        for (name, arena, _) in &arenas {
            let n = arena.rows.len();
            // A node is virtualizable when it holds exactly one token child.
            // The kept rows stay full rows, so this measures the shorter array
            // and the bit-31 branch, not a narrower row.
            let mut compact: Vec<TodayRow> = Vec::new();
            let mut ids: Vec<u32> = Vec::with_capacity(n);
            for (i, r) in arena.rows.iter().enumerate() {
                let (s, cn) = r.children;
                match (cn == 1).then(|| arena.close[s as usize]) {
                    Some(Ref::Token(t)) => {
                        ids.push(0x8000_0000 | ((r.kind as u32) << 26) | (t << 6));
                    }
                    _ => {
                        ids.push(compact.len() as u32);
                        compact.push(arena.today[i]);
                    }
                }
            }
            let plain: Vec<u32> = (0..n as u32).collect();
            g.throughput(Throughput::Elements(n as u64));

            g.bench_function(BenchmarkId::new("all_rows", name), |b| {
                b.iter(|| {
                    let mut acc = 0u64;
                    for &i in plain.iter() {
                        let r = &arena.today[i as usize];
                        acc += r.kind as u64 + r.span.start as u64;
                    }
                    black_box(acc)
                })
            });
            g.bench_function(BenchmarkId::new("virtual_compacted", name), |b| {
                b.iter(|| {
                    let mut acc = 0u64;
                    for &id in ids.iter() {
                        if id & 0x8000_0000 != 0 {
                            let kind = (id >> 26) & 0x1f;
                            let tok = (id >> 6) & 0xf_ffff;
                            acc += kind as u64 + arena.tokens[tok as usize].span.start as u64;
                        } else {
                            let r = compact[id as usize];
                            acc += r.kind as u64 + r.span.start as u64;
                        }
                    }
                    black_box(acc)
                })
            });
        }
    }
}

/// Section 4.2's resolution: two array reads, no walking.
#[inline]
fn derive(tokens: &[Token], range: (u32, u32)) -> u32 {
    let (start, len) = range;
    if len == 0 {
        return 0;
    }
    let first = tokens[start as usize].span.start;
    let last = tokens[(start + len - 1) as usize].span.end;
    last - first
}

#[inline]
fn id_of(r: Ref) -> u32 {
    match r {
        Ref::Token(t) | Ref::Tree(t) => t,
    }
}

criterion_group!(incremental, benches);
criterion_main!(incremental);
