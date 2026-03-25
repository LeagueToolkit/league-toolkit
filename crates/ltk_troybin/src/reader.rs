//! Binary reader for INIBIN v1 and v2 (troybin) files.
//!
//! Format based on Leischii's TroybinConverter and Rey's TroybinEditor.

use std::io::{self, Cursor, Read};

use crate::error::TroybinError;
use crate::types::{RawEntry, StorageType, Value};

// ── Cursor wrapper ──────────────────────────────────────────────────────────

pub(crate) struct BinReader {
    cursor: Cursor<Vec<u8>>,
}

impl BinReader {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    pub fn read_u8(&mut self) -> io::Result<u8> {
        let mut b = [0u8; 1];
        self.cursor.read_exact(&mut b)?;
        Ok(b[0])
    }
    pub fn read_u16_le(&mut self) -> io::Result<u16> {
        let mut b = [0u8; 2];
        self.cursor.read_exact(&mut b)?;
        Ok(u16::from_le_bytes(b))
    }
    pub fn read_i16_le(&mut self) -> io::Result<i16> {
        let mut b = [0u8; 2];
        self.cursor.read_exact(&mut b)?;
        Ok(i16::from_le_bytes(b))
    }
    pub fn read_u32_le(&mut self) -> io::Result<u32> {
        let mut b = [0u8; 4];
        self.cursor.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }
    pub fn read_i32_le(&mut self) -> io::Result<i32> {
        let mut b = [0u8; 4];
        self.cursor.read_exact(&mut b)?;
        Ok(i32::from_le_bytes(b))
    }
    pub fn read_f32_le(&mut self) -> io::Result<f32> {
        let mut b = [0u8; 4];
        self.cursor.read_exact(&mut b)?;
        Ok(f32::from_le_bytes(b))
    }
    pub fn read_bytes(&mut self, n: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; n];
        self.cursor.read_exact(&mut buf)?;
        Ok(buf)
    }
    pub fn skip(&mut self, n: usize) -> io::Result<()> {
        let mut buf = vec![0u8; n];
        self.cursor.read_exact(&mut buf)?;
        Ok(())
    }
}

// ── V1 reader ───────────────────────────────────────────────────────────────

fn sanitize_str(s: &str) -> Value {
    if s == "true" {
        return Value::Int(1);
    }
    if s == "false" {
        return Value::Int(0);
    }
    if s.eq_ignore_ascii_case("nan") {
        return Value::Float(f64::NAN);
    }

    // Try parsing as space-separated numbers (vectors)
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() > 1 {
        let nums: Vec<f64> = parts.iter().filter_map(|p| p.parse().ok()).collect();
        if nums.len() == parts.len() {
            return Value::Vec(nums);
        }
    }

    // Single number
    if let Ok(v) = s.parse::<f64>() {
        return Value::Float(v);
    }

    Value::String(s.to_string())
}

pub(crate) fn read_v1(r: &mut BinReader) -> Result<Vec<RawEntry>, TroybinError> {
    r.skip(3)?; // 3 unknown bytes
    let entry_count = r.read_u32_le()? as usize;
    let data_count = r.read_u32_le()? as usize;

    let mut offsets = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let h = r.read_u32_le()?;
        let o = r.read_u32_le()?;
        offsets.push((h, o as usize));
    }

    let data = r.read_bytes(data_count)?;
    let mut result = Vec::with_capacity(entry_count);

    for &(hash, offset) in &offsets {
        let mut o = offset;
        let mut s = String::new();
        while o < data.len() && data[o] != 0 {
            s.push(data[o] as char);
            o += 1;
        }
        result.push(RawEntry {
            hash,
            value: sanitize_str(&s),
            storage: StorageType::OldFormat,
        });
    }
    Ok(result)
}

// ── V2 readers ──────────────────────────────────────────────────────────────

fn read_bools(r: &mut BinReader) -> Result<Vec<RawEntry>, TroybinError> {
    let num = r.read_u16_le()? as usize;
    let mut keys = Vec::with_capacity(num);
    for _ in 0..num {
        keys.push(r.read_u32_le()?);
    }
    let bytes_count = num / 8 + if !num.is_multiple_of(8) { 1 } else { 0 };
    let bools = r.read_bytes(bytes_count)?;
    let mut result = Vec::with_capacity(num);
    for (j, &key) in keys.iter().enumerate() {
        let bit = (bools[j / 8] >> (j % 8)) & 1;
        result.push(RawEntry {
            hash: key,
            value: Value::Int(bit as i32),
            storage: StorageType::Bool,
        });
    }
    Ok(result)
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
enum NumFmt {
    I32,
    F32,
    U8,
    I16,
    U16,
}

fn read_numbers(
    r: &mut BinReader,
    fmt: NumFmt,
    count: usize,
    mul: f64,
    storage: StorageType,
) -> Result<Vec<RawEntry>, TroybinError> {
    let num = r.read_u16_le()? as usize;
    let mut keys = Vec::with_capacity(num);
    for _ in 0..num {
        keys.push(r.read_u32_le()?);
    }
    let mut result = Vec::with_capacity(num);
    for &key in &keys {
        let mut vals = Vec::with_capacity(count);
        for _ in 0..count {
            let raw: f64 = match fmt {
                NumFmt::I32 => r.read_i32_le()? as f64,
                NumFmt::F32 => r.read_f32_le()? as f64,
                NumFmt::U8 => r.read_u8()? as f64,
                NumFmt::I16 => r.read_i16_le()? as f64,
                NumFmt::U16 => r.read_u16_le()? as f64,
            };
            vals.push(raw * mul);
        }
        let value = if count == 1 && mul == 1.0 {
            match fmt {
                NumFmt::I32 | NumFmt::I16 | NumFmt::U16 => Value::Int(vals[0] as i32),
                _ => Value::Float(vals[0]),
            }
        } else if count == 1 {
            Value::Float(vals[0])
        } else {
            Value::Vec(vals)
        };
        result.push(RawEntry {
            hash: key,
            value,
            storage,
        });
    }
    Ok(result)
}

fn read_strings(r: &mut BinReader, strings_length: usize) -> Result<Vec<RawEntry>, TroybinError> {
    let num = r.read_u16_le()? as usize;
    let mut keys = Vec::with_capacity(num);
    for _ in 0..num {
        keys.push(r.read_u32_le()?);
    }
    // Read offsets (u16 per string)
    let mut offsets = Vec::with_capacity(num);
    for _ in 0..num {
        offsets.push(r.read_u16_le()? as usize);
    }
    let data = r.read_bytes(strings_length)?;
    let mut result = Vec::with_capacity(num);
    for i in 0..num {
        let mut o = offsets[i];
        let mut s = String::new();
        while o < data.len() && data[o] != 0 {
            s.push(data[o] as char);
            o += 1;
        }
        result.push(RawEntry {
            hash: keys[i],
            value: Value::String(s),
            storage: StorageType::StringBlock,
        });
    }
    Ok(result)
}

pub(crate) fn read_v2(r: &mut BinReader) -> Result<Vec<RawEntry>, TroybinError> {
    let strings_length = r.read_u16_le()? as usize;
    let mut flags = r.read_u16_le()?;
    if flags == 0 {
        flags = r.read_u16_le()?;
    }

    let mut target = Vec::new();

    for i in 0u16..16 {
        if flags & (1 << i) == 0 {
            continue;
        }
        let entries = match i {
            0 => read_numbers(r, NumFmt::I32, 1, 1.0, StorageType::Int32)?,
            1 => read_numbers(r, NumFmt::F32, 1, 1.0, StorageType::Float32)?,
            2 => read_numbers(r, NumFmt::U8, 1, 0.1, StorageType::U8Scaled)?,
            3 => read_numbers(r, NumFmt::I16, 1, 1.0, StorageType::Int16)?,
            4 => read_numbers(r, NumFmt::U8, 1, 1.0, StorageType::U8)?,
            5 => read_bools(r)?,
            6 => read_numbers(r, NumFmt::U8, 3, 0.1, StorageType::U8x3Scaled)?,
            7 => read_numbers(r, NumFmt::F32, 3, 1.0, StorageType::Float32x3)?,
            8 => read_numbers(r, NumFmt::U8, 2, 0.1, StorageType::U8x2Scaled)?,
            9 => read_numbers(r, NumFmt::F32, 2, 1.0, StorageType::Float32x2)?,
            10 => read_numbers(r, NumFmt::U8, 4, 0.1, StorageType::U8x4Scaled)?,
            11 => read_numbers(r, NumFmt::F32, 4, 1.0, StorageType::Float32x4)?,
            12 => read_strings(r, strings_length)?,
            13 => read_numbers(r, NumFmt::I32, 1, 1.0, StorageType::Int32Long)?,
            _ => Vec::new(),
        };
        target.extend(entries);
    }
    Ok(target)
}

/// Read a troybin binary buffer, returning version + raw entries.
pub(crate) fn read_binary(data: &[u8]) -> Result<(u8, Vec<RawEntry>), TroybinError> {
    if data.is_empty() {
        return Err(TroybinError::Empty);
    }

    let mut reader = BinReader::new(data.to_vec());
    let version = reader.read_u8()?;

    let entries = match version {
        2 => read_v2(&mut reader)?,
        1 => read_v1(&mut reader)?,
        _ => return Err(TroybinError::UnknownVersion(version)),
    };

    Ok((version, entries))
}
