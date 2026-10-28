//! Slices the media of a package by its entry table, without `ltk_wwise::package`.

/// Returns the name and bytes of every entry of the package `data`.
///
/// Returns `None` when a record or a media lies outside `data`.
pub fn entries(data: &[u8]) -> Option<Vec<(String, &[u8])>> {
    let count = read_u32(data, 8)?;
    (0..count)
        .map(|i| {
            let record = read_u32(data, 12 + 4 * i as usize)? as usize;
            let data_offset = read_u32(data, record)? as usize;
            let data_size = read_u32(data, record + 4)? as usize;
            let name_length = read_u32(data, record + 8)? as usize;
            let name_bytes = data.get(record + 12..record + 12 + 2 * name_length)?;
            let name = String::from_utf16_lossy(
                &name_bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|unit| u16::from_le_bytes(*unit))
                    .collect::<Vec<_>>(),
            );
            let media = data.get(data_offset..data_offset + data_size)?;
            Some((name, media))
        })
        .collect()
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}
