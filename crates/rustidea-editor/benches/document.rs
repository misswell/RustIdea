use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rustidea_editor::{Change, Document, DocumentId, EditSource, Transaction};

fn text(lines: usize) -> String {
    let mut output = String::with_capacity(lines * 40);
    for line in 0..lines {
        output.push_str(&format!("fn generated_{line}() -> usize {{ {line} }}\n"));
    }
    output
}

fn document(lines: usize) -> Document {
    Document::new(DocumentId::new(1), &text(lines))
}

fn edit_one_line(document: &mut Document, line: u32) {
    let version = document.version();
    let start = document.line_start(line);
    let change = Change::replace(start..start, "x");
    document
        .apply(Transaction::new(version, vec![change], EditSource::Local))
        .unwrap();
}

fn viewport_extraction(document: &Document, first: u32) {
    for line in first..first + 60 {
        let _ = document.line(line.min(document.line_count() - 1));
    }
}

fn document_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("document_engine");
    for lines in [100_000usize, 1_000_000] {
        group.bench_with_input(
            BenchmarkId::new("single_edit", lines),
            &lines,
            |b, lines| {
                let mut document = document(*lines);
                let target = (*lines as u32 / 2).saturating_sub(1);
                b.iter(|| edit_one_line(&mut document, target));
            },
        );
        group.bench_with_input(BenchmarkId::new("undo_redo", lines), &lines, |b, lines| {
            let mut document = document(*lines);
            let target = (*lines as u32 / 2).saturating_sub(1);
            b.iter(|| {
                edit_one_line(&mut document, target);
                document.undo().unwrap();
                document.redo().unwrap();
            });
        });
        group.bench_with_input(
            BenchmarkId::new("viewport_line_reads", lines),
            &lines,
            |b, lines| {
                let document = document(*lines);
                let first = (*lines as u32 / 2).saturating_sub(30);
                b.iter(|| viewport_extraction(&document, first));
            },
        );
    }
    group.finish();
}

criterion_group!(benches, document_benchmarks);
criterion_main!(benches);
