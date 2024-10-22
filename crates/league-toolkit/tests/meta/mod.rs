use std::io::Cursor;

use insta::assert_ron_snapshot;
use league_toolkit::core::meta::BinTree;
#[test]
pub fn read() {
    let mut r = Cursor::new(include_bytes!("bins/leona_small.bin"));
    let bin = BinTree::from_reader(&mut r).unwrap();
    insta::with_settings!({sort_maps => true}, {
            assert_ron_snapshot!(bin);
    });
}
