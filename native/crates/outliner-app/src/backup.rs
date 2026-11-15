//! Backup and import (the web's `ui/backup.ts`, `ui/zip.ts` and `importOpmlFile`): every document as plain text
//! and OPML in a zip, and documents made from OPML or text files.

use outliner_core::commands::Portable;
use outliner_core::export::{export_opml, export_text, ExportScope, IndentStyle, OpmlExportOptions, TextExportOptions};
use outliner_core::{dates, NodeId, ROOT};

use crate::store::State;

/// A file name without the characters file systems refuse, not starting with a dot, at most 120 characters
/// (`safe` on the web).
fn safe(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in s.chars() {
        if matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || (c as u32) < 0x20 {
            if !gap {
                out.push('_');
            }
            gap = true;
        } else {
            out.push(c);
            gap = false;
        }
    }
    let dots = out.len() - out.trim_start_matches('.').len();
    if dots > 0 {
        out = format!("_{}", &out[dots..]);
    }
    // 120 UTF-16 units, as JS's `slice`
    let mut cut = String::new();
    let mut units = 0;
    for c in out.chars() {
        units += c.len_utf16();
        if units > 120 {
            break;
        }
        cut.push(c);
    }
    if cut.is_empty() { "Untitled".to_owned() } else { cut }
}

/// The backup's files: each document as `<title>.txt` (its title, then its items indented by spaces) and
/// `<title>.opml`, in the sidebar's order, names made unique ignoring case (`backupZip`).
pub fn backup_files(s: &State) -> Vec<(String, String)> {
    let mut used: Vec<String> = Vec::new();
    let mut files = Vec::new();
    for id in &s.files {
        let Some(t) = s.tree_of(id) else { continue };
        let title = s.title(id);
        let name = safe(&title);
        let mut base = name.clone();
        let mut i = 2;
        while used.contains(&base.to_lowercase()) {
            base = format!("{name} ({i})");
            i += 1;
        }
        used.push(base.to_lowercase());
        let ids: Vec<NodeId> = t.get(ROOT).map(|n| n.children.clone()).unwrap_or_default();
        let text = export_text(t, &ids, &ExportScope::default(), &TextExportOptions { indent: IndentStyle::Spaces, ..Default::default() });
        files.push((format!("{base}.txt"), format!("{title}\n{text}")));
        let opml = export_opml(t, &ids, &ExportScope::default(), &OpmlExportOptions { title: Some(title), ..Default::default() });
        files.push((format!("{base}.opml"), opml));
    }
    files
}

/// The backup's file name on day `now` (`outliner-backup-YYYY-MM-DD.zip`, the UTC day as the web's ISO date).
pub fn backup_name(now: i64) -> String {
    let (y, m, d) = dates::civil_from_days(now.div_euclid(86_400_000));
    format!("outliner-backup-{y:04}-{m:02}-{d:02}.zip")
}

/// CRC-32 (IEEE), as zip entries carry it.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for b in data {
        c ^= u32::from(*b);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    c ^ 0xffff_ffff
}

/// A zip of `files` (stored, not compressed; UTF-8 names) dated `now` in local time (`zip` on the web).
pub fn zip(files: &[(String, String)], now: i64) -> Vec<u8> {
    let f = dates::fields(now);
    let time = ((f.hours << 11) | (f.minutes << 5) | (f.seconds >> 1)) as u16;
    let day = (((f.year - 1980) << 9) | ((f.month0 + 1) << 5) | f.day) as u16;
    let mut out = Vec::new();
    let mut central = Vec::new();
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    for (name, data) in files {
        let (name, data) = (name.as_bytes(), data.as_bytes());
        let crc = crc32(data);
        let offset = out.len() as u32;
        le32(&mut out, 0x0403_4b50);
        le16(&mut out, 20);
        le16(&mut out, 0x0800);
        le16(&mut out, 0);
        le16(&mut out, time);
        le16(&mut out, day);
        le32(&mut out, crc);
        le32(&mut out, data.len() as u32);
        le32(&mut out, data.len() as u32);
        le16(&mut out, name.len() as u16);
        le16(&mut out, 0);
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        le32(&mut central, 0x0201_4b50);
        le16(&mut central, 20);
        le16(&mut central, 20);
        le16(&mut central, 0x0800);
        le16(&mut central, 0);
        le16(&mut central, time);
        le16(&mut central, day);
        le32(&mut central, crc);
        le32(&mut central, data.len() as u32);
        le32(&mut central, data.len() as u32);
        le16(&mut central, name.len() as u16);
        central.extend_from_slice(&[0; 12]);
        le32(&mut central, offset);
        central.extend_from_slice(name);
    }
    let at = out.len() as u32;
    let size = central.len() as u32;
    out.extend_from_slice(&central);
    le32(&mut out, 0x0605_4b50);
    le32(&mut out, 0);
    le16(&mut out, files.len() as u16);
    le16(&mut out, files.len() as u16);
    le32(&mut out, size);
    le32(&mut out, at);
    le16(&mut out, 0);
    out
}

/// An imported file's document: its title and items (`importOpmlFile`): OPML (`.opml`, `.xml`) with its title
/// or the file's name, else indented text; `None` for OPML that cannot be read.
pub fn import_file(name: &str, text: &str) -> Option<(String, Vec<Portable>)> {
    let base = match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    };
    let lower = name.to_lowercase();
    if lower.ends_with(".opml") || lower.ends_with(".xml") {
        let doc = outliner_core::export::opml_to_portable(text)?;
        let title = if doc.title.is_empty() { base.to_owned() } else { doc.title };
        return Some((title, doc.items));
    }
    Some((base.to_owned(), outliner_core::paste::parse_plain_text(text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_and_zip_are_the_formats() {
        assert_eq!(crc32(b"hello"), 0x3610_a686);
        assert_eq!(crc32(b""), 0);
        let z = zip(&[("a.txt".into(), "hi".into()), ("한.opml".into(), "x".into())], 0);
        // local headers, then the central directory, then its end with the count and offset
        assert_eq!(&z[..4], &[0x50, 0x4b, 0x03, 0x04]);
        let end = z.len() - 22;
        assert_eq!(&z[end..end + 4], &[0x50, 0x4b, 0x05, 0x06]);
        assert_eq!(u16::from_le_bytes([z[end + 10], z[end + 11]]), 2);
        let cd = u32::from_le_bytes(z[end + 16..end + 20].try_into().unwrap()) as usize;
        assert_eq!(&z[cd..cd + 4], &[0x50, 0x4b, 0x01, 0x02]);
        // the second entry's name, UTF-8, after its local header
        let second = 30 + 5 + 2;
        assert_eq!(&z[second + 30..second + 30 + "한.opml".len()], "한.opml".as_bytes());
        assert_eq!(u32::from_le_bytes(z[14..18].try_into().unwrap()), crc32(b"hi"));
    }

    #[test]
    fn names_are_safe_and_unique() {
        assert_eq!(safe("a/b: c?"), "a_b_ c_");
        assert_eq!(safe("..hidden"), "_hidden");
        assert_eq!(safe(""), "Untitled");
        assert_eq!(backup_name(0), "outliner-backup-1970-01-01.zip");
    }

    #[test]
    fn files_import_as_documents() {
        let (t, items) = import_file("list.txt", "a\n    b\nc").unwrap();
        assert_eq!(t, "list");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].children.len(), 1);
        let opml = "<?xml version=\"1.0\"?><opml version=\"2.0\"><head><title>Plan</title></head><body><outline text=\"x\"/></body></opml>";
        let (t, items) = import_file("w.opml", opml).unwrap();
        assert_eq!((t.as_str(), items.len()), ("Plan", 1));
        assert!(import_file("bad.opml", "nope").is_none());
    }
}
