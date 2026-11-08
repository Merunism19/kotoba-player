//! Builds small Yomitan-format zips in memory.
#![allow(dead_code)]

use std::io::{Cursor, Write};

use serde_json::{Value, json};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// A zip from (name, bytes) pairs; alternate members deflated and stored, to exercise both.
pub fn zip_of(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    for (i, (name, data)) in files.iter().enumerate() {
        let method = if i % 2 == 0 {
            CompressionMethod::Deflated
        } else {
            CompressionMethod::Stored
        };
        w.start_file(
            *name,
            SimpleFileOptions::default().compression_method(method),
        )
        .unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

pub fn index_json(title: &str, revision: &str, extra: Value) -> Vec<u8> {
    let mut v = json!({"title": title, "format": 3, "revision": revision});
    if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        for (k, x) in e {
            o.insert(k.clone(), x.clone());
        }
    }
    serde_json::to_vec(&v).unwrap()
}

/// (expression, reading, definition tags, rules, score, glossary)
pub type TermSpec<'a> = (&'a str, &'a str, &'a str, &'a str, i64, Value);

/// A term dictionary.
pub fn term_zip(title: &str, rows: &[TermSpec]) -> Vec<u8> {
    let bank: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(i, (e, r, t, ru, s, g))| json!([e, r, t, ru, s, g, i + 1, ""]))
        .collect();
    zip_of(&[
        ("index.json", index_json(title, "1", json!({}))),
        ("term_bank_1.json", serde_json::to_vec(&bank).unwrap()),
        ("tag_bank_1.json", b"[]".to_vec()),
    ])
}

/// A frequency dictionary from (expression, data) pairs; `data` is any of the Yomitan shapes.
pub fn freq_zip(title: &str, rows: &[(&str, Value)], extra_index: Value) -> Vec<u8> {
    let bank: Vec<Value> = rows.iter().map(|(e, d)| json!([e, "freq", d])).collect();
    zip_of(&[
        ("index.json", index_json(title, "2024.1", extra_index)),
        ("term_meta_bank_1.json", serde_json::to_vec(&bank).unwrap()),
    ])
}

pub fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}
