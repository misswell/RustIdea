use std::path::PathBuf;

fn main() {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: write_heap_fixture <output .hprof>");
    ide_heap::testing::sample().write_to(&path);
    println!("{}", path.display());
}
