use std::io::{Cursor, Read};
pub(super) fn extract(bytes: &[u8], name: &str, mime: &str) -> Result<(String, String), String> {
    if bytes.len() > super::fetch::MAX_BYTES {
        return Err("Document exceeds 20 MiB".into());
    }
    let name = name.split('?').next().unwrap_or(name).to_lowercase();
    let (title, text) = if bytes.starts_with(b"%PDF-") {
        (
            String::new(),
            pdf_extract::extract_text_from_mem(bytes)
                .map_err(|e| format!("PDF extraction: {e}"))?,
        )
    } else if bytes.starts_with(b"PK\x03\x04") {
        (String::new(), docx(bytes)?)
    } else if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0]) {
        return Err(
            "Legacy .doc requires conversion to .docx; encrypted Office files are not supported"
                .into(),
        );
    } else {
        let text =
            std::str::from_utf8(bytes).map_err(|_| "Document is not UTF-8 text, PDF or DOCX")?;
        if mime.contains("html")
            || name.ends_with(".html")
            || text
                .trim_start()
                .to_lowercase()
                .starts_with("<!doctype html")
            || text.trim_start().starts_with("<html")
        {
            html(text)
        } else {
            (String::new(), text.to_owned())
        }
    };
    if text.trim().is_empty() {
        return Err(
            "No readable text found; scanned PDF/image-only documents need OCR before import"
                .into(),
        );
    }
    if text.len() > 4 * 1024 * 1024 {
        return Err("Extracted text exceeds 4 MiB; split the document".into());
    }
    if text.contains('\0') {
        return Err("Unsupported binary document".into());
    }
    Ok((title, text.trim().into()))
}
fn html(text: &str) -> (String, String) {
    let document = scraper::Html::parse_document(text);
    let title = document
        .select(&scraper::Selector::parse("title").unwrap())
        .next()
        .map(|n| n.text().collect::<String>())
        .unwrap_or_default();
    let selector = scraper::Selector::parse("main,article").unwrap();
    let root = document
        .select(&selector)
        .next()
        .unwrap_or_else(|| document.root_element());
    let text = root
        .descendants()
        .filter_map(|node| {
            let text = node.value().as_text()?;
            if node.ancestors().any(|p| {
                p.value().as_element().is_some_and(|e| {
                    matches!(
                        e.name(),
                        "script" | "style" | "noscript" | "nav" | "footer" | "head"
                    )
                })
            }) {
                return None;
            }
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        })
        .collect::<Vec<_>>()
        .join("\n");
    (title, text)
}
fn docx(bytes: &[u8]) -> Result<String, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let file = archive
        .by_name("word/document.xml")
        .map_err(|_| "DOCX has no word/document.xml")?;
    if file.size() > 8 * 1024 * 1024 {
        return Err("DOCX text XML exceeds 8 MiB".into());
    }
    let mut xml = String::new();
    file.take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;
    if xml.len() > 8 * 1024 * 1024 {
        return Err("DOCX expansion limit exceeded".into());
    }
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut text = String::new();
    let mut inside = false;
    loop {
        use quick_xml::events::Event;
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) if e.local_name().as_ref() == b"t" => inside = true,
            Event::End(e) => {
                if e.local_name().as_ref() == b"t" {
                    inside = false;
                }
                if e.local_name().as_ref() == b"p" {
                    text.push('\n');
                }
            }
            Event::Text(e) if inside => text.push_str(&e.decode().map_err(|e| e.to_string())?),
            Event::GeneralRef(e) if inside => {
                let entity = e.decode().map_err(|e| e.to_string())?;
                text.push_str(
                    &quick_xml::escape::unescape(&format!("&{entity};"))
                        .map_err(|e| e.to_string())?,
                );
            }
            Event::Empty(e) if matches!(e.local_name().as_ref(), b"tab" | b"br") => text.push('\n'),
            Event::DocType(_) => return Err("DOCX DTD is not allowed".into()),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(text)
}
#[cfg(test)]
mod tests {
    #[test]
    fn pdf_text_is_extracted() {
        let stream = "BT /F1 12 Tf 10 80 Td (Crabot PDF document) Tj ET";
        let objects=["<< /Type /Catalog /Pages 2 0 R >>".to_string(),"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".into(),"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),format!("<< /Length {} >>\nstream\n{stream}\nendstream",stream.len())];
        let mut pdf = "%PDF-1.4\n".to_string();
        let mut offsets = vec![0];
        for (i, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", i + 1));
        }
        let xref = pdf.len();
        pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
        for offset in offsets.iter().skip(1) {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!(
            "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF"
        ));
        assert!(
            super::extract(pdf.as_bytes(), "a.pdf", "")
                .unwrap()
                .1
                .contains("Crabot PDF document")
        );
    }
    #[test]
    fn docx_extracts_paragraphs_entities_and_tabs() {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(br#"<?xml version="1.0"?><w:document xmlns:w="word"><w:p><w:r><w:t>Hello &amp; world</w:t><w:tab/><w:t>second</w:t></w:r></w:p></w:document>"#).unwrap();
        let data = zip.finish().unwrap().into_inner();
        assert_eq!(
            super::extract(&data, "doc.docx", "").unwrap().1,
            "Hello & world\nsecond"
        );
    }
    #[test]
    fn html_drops_active_content() {
        let (t, s) = super::extract(
            b"<title>T</title><main>Hello<script>bad()</script><p>World</p></main>",
            "page",
            "text/html",
        )
        .unwrap();
        assert_eq!(t, "T");
        assert_eq!(s, "Hello\nWorld");
    }
    #[test]
    fn rejects_binary_and_empty() {
        assert!(super::extract(&[255, 254], "file", "").is_err());
        assert!(super::extract(b" ", "file", "").is_err());
    }
}
