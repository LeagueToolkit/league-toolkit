use insta::assert_debug_snapshot;
use league_toolkit::core::animation::RigResource;
use std::io::Cursor;

#[test]
pub fn read() {
    let mut r = Cursor::new(include_bytes!("jackinthebox.skl"));
    let skl = RigResource::from_reader(&mut r).unwrap();
    assert_debug_snapshot!(skl);
}
