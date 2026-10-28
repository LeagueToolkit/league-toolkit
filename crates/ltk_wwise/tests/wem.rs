mod common;
mod wpk_media;

use std::time::Duration;

use ltk_wwise::wem::{WemCodec, WemInfo};

/// Returns the bytes of the entry `name` of the package `data`.
fn media<'a>(data: &'a [u8], name: &str) -> &'a [u8] {
    wpk_media::entries(data)
        .expect("package entry table is in bounds")
        .into_iter()
        .find(|(entry, _)| entry == name)
        .unwrap_or_else(|| panic!("package has no entry {name}"))
        .1
}

#[test]
fn reads_the_format_of_a_packaged_vorbis_wem() {
    let package = common::fixture("package.wpk");
    let wem = media(&package, "380333190.wem");

    let info = WemInfo::from_bytes(wem).unwrap();

    assert_eq!(
        info,
        WemInfo {
            codec: WemCodec::VORBIS,
            channels: 1,
            sample_rate: 24_000,
            average_bytes_per_second: 6_055,
            data_size: 2_176,
        }
    );
    // 2176 bytes at 6055 bytes per second.
    assert_eq!(info.duration(), Some(Duration::from_nanos(359_372_419)));
}

/// A PCM `fmt ` body: 2 channels, 48000 Hz, 192000 bytes per second.
const PCM_FMT: [u8; 16] = [
    0x01, 0x00, 0x02, 0x00, 0x80, 0xBB, 0x00, 0x00, 0x00, 0xEE, 0x02, 0x00, 0x04, 0x00, 0x10, 0x00,
];

/// Returns a RIFF `WAVE` file of the chunks `chunks`, each body padded to an even size.
fn riff(chunks: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
    let mut body = b"WAVE".to_vec();
    for (tag, data) in chunks {
        body.extend_from_slice(*tag);
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(data);
        if data.len() % 2 == 1 {
            body.push(0);
        }
    }
    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&body);
    file
}

fn wem_error(data: &[u8]) -> &'static str {
    match WemInfo::from_bytes(data) {
        Err(ltk_wwise::Error::Wem(reason)) => reason,
        other => panic!("expected Error::Wem, got {other:?}"),
    }
}

#[test]
fn an_odd_sized_chunk_before_fmt_is_skipped_with_its_pad_byte() {
    let file = riff(&[
        (b"junk", &[0xAA; 3]),
        (b"fmt ", &PCM_FMT),
        (b"data", &[0; 9600]),
    ]);

    let info = WemInfo::from_bytes(&file).unwrap();

    assert_eq!(info.codec, WemCodec::PCM);
    assert_eq!(info.channels, 2);
    assert_eq!(info.sample_rate, 48_000);
    assert_eq!(info.data_size, 9_600);
    assert_eq!(info.duration(), Some(Duration::from_millis(50)));
}

#[test]
fn a_zero_byte_rate_has_no_duration() {
    let mut fmt = PCM_FMT;
    fmt[8..12].fill(0);
    let file = riff(&[(b"fmt ", &fmt), (b"data", &[0; 4])]);

    assert_eq!(WemInfo::from_bytes(&file).unwrap().duration(), None);
}

#[test]
fn a_file_without_riff_is_not_a_wem() {
    let mut file = riff(&[(b"fmt ", &PCM_FMT), (b"data", &[])]);
    file[..4].copy_from_slice(b"RIFX");

    wem_error(&file);
    wem_error(b"RI");
}

#[test]
fn a_file_without_wave_is_not_a_wem() {
    let mut file = riff(&[(b"fmt ", &PCM_FMT), (b"data", &[])]);
    file[8..12].copy_from_slice(b"AVI ");

    wem_error(&file);
}

#[test]
fn a_file_without_fmt_is_not_a_wem() {
    wem_error(&riff(&[(b"data", &[0; 8])]));
    wem_error(&riff(&[]));
}

#[test]
fn a_file_without_data_is_not_a_wem() {
    wem_error(&riff(&[(b"fmt ", &PCM_FMT)]));
}

#[test]
fn a_fmt_chunk_shorter_than_its_fields_is_not_a_wem() {
    wem_error(&riff(&[(b"fmt ", &PCM_FMT[..14]), (b"data", &[])]));
}

#[test]
fn a_file_cut_inside_the_fmt_body_is_not_a_wem() {
    let file = riff(&[(b"fmt ", &PCM_FMT), (b"data", &[])]);

    assert_eq!(
        wem_error(&file[..30]),
        "fmt chunk ends after the end of the file"
    );
}

#[test]
fn a_file_cut_inside_a_chunk_header_is_not_a_wem() {
    let file = riff(&[(b"fmt ", &PCM_FMT), (b"data", &[])]);

    // Cut inside the `fmt ` header, then inside the `data` header.
    wem_error(&file[..16]);
    wem_error(&file[..40]);
}

#[test]
fn a_shipped_wem_cut_inside_its_data_header_is_not_a_wem() {
    let package = common::fixture("package.wpk");
    let wem = media(&package, "470358536.wem");

    // `data` starts at byte 162 of this WEM.
    wem_error(&wem[..166]);
    assert_eq!(WemInfo::from_bytes(&wem[..170]).unwrap().data_size, 4_276);
}
