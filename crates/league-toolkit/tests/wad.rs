use insta::assert_ron_snapshot;
use league_toolkit::core::wad::Wad;
use std::io::Cursor;

#[test]
fn read() {
    let r = Cursor::new(include_bytes!("wads/UI.en_US.wad.client.v3-4"));
    let wad = Wad::mount(r).unwrap();

    insta::with_settings!({sort_maps => true}, {
        assert_ron_snapshot!(wad.chunks());
    });
}
