use std::fs;
use std::io::Read;
use std::path::Path;

pub fn extract_text(file_path: &str) -> Result<ExtractedDocument, String> {
    let path = Path::new(file_path);
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    let file_name = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未知文档")
        .to_string();

    match ext.as_str() {
        "txt" => extract_txt(file_path, &file_name),
        "docx" => extract_docx(file_path, &file_name),
        _ => Err(format!("暂不支持的格式：.{}，请使用 .txt 或 .docx", ext)),
    }
}

pub struct ExtractedDocument {
    pub title: String,
    pub file_type: String,
    pub word_count: u32,
    pub full_text: String,
}

fn extract_txt(file_path: &str, title: &str) -> Result<ExtractedDocument, String> {
    let bytes = fs::read(file_path).map_err(|e| format!("读取文件失败: {}", e))?;

    let text = if let Ok(utf8_text) = String::from_utf8(bytes.clone()) {
        utf8_text
    } else {
        let (decoded, _, had_errors) = encoding_rs::GBK.decode(&bytes);
        if had_errors {
            String::from_utf8_lossy(&bytes).to_string()
        } else {
            decoded.into_owned()
        }
    };

    let word_count = count_chinese_words(&text);

    Ok(ExtractedDocument {
        title: title.to_string(),
        file_type: "txt".to_string(),
        word_count,
        full_text: text,
    })
}

fn extract_docx(file_path: &str, title: &str) -> Result<ExtractedDocument, String> {
    let file = fs::File::open(file_path).map_err(|e| format!("读取文件失败: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("解析docx失败: {}", e))?;

    let mut doc_xml = archive.by_name("word/document.xml")
        .map_err(|_| "无效的docx文件：未找到word/document.xml".to_string())?;

    let mut xml = String::new();
    doc_xml.read_to_string(&mut xml).map_err(|e| format!("读取docx内容失败: {}", e))?;

    let text = extract_text_from_docx_xml(&xml);
    let word_count = count_chinese_words(&text);

    Ok(ExtractedDocument {
        title: title.to_string(),
        file_type: "docx".to_string(),
        word_count,
        full_text: text,
    })
}

fn extract_text_from_docx_xml(xml: &str) -> String {
    let mut result = String::new();
    let mut in_text_tag = false;
    let mut tag_buffer = String::new();

    for ch in xml.chars() {
        if ch == '<' {
            if in_text_tag && !tag_buffer.is_empty() {
                result.push_str(&tag_buffer);
                tag_buffer.clear();
            }
            in_text_tag = false;
            tag_buffer.clear();
            tag_buffer.push(ch);
        } else if ch == '>' {
            tag_buffer.push(ch);
            let tag = &tag_buffer;
            if tag == "<w:t>" || tag == "<w:t xml:space=\"preserve\">" || tag.starts_with("<w:t ") {
                in_text_tag = true;
            } else if tag == "</w:t>" {
                in_text_tag = false;
            } else if tag == "</w:p>" {
                if !result.is_empty() && !result.ends_with('\n') {
                    result.push('\n');
                }
            }
            tag_buffer.clear();
        } else if in_text_tag {
            tag_buffer.push(ch);
        } else {
            tag_buffer.push(ch);
        }
    }

    result.trim().to_string()
}

pub fn count_chinese_words(text: &str) -> u32 {
    let mut count = 0u32;
    for ch in text.chars() {
        if ch as u32 >= 0x4E00 && ch as u32 <= 0x9FFF
            || ch as u32 >= 0x3400 && ch as u32 <= 0x4DBF
            || ch as u32 >= 0x2E80 && ch as u32 <= 0x2FDF
            || ch as u32 >= 0x3000 && ch as u32 <= 0x303F
        {
            count += 1;
        } else if ch.is_alphabetic() {
            count += 1;
        }
    }
    count
}

pub fn search_in_text(full_text: &str, keyword: &str, context_chars: usize) -> String {
    if keyword.is_empty() || full_text.is_empty() {
        return String::new();
    }

    let lower_text = full_text.to_lowercase();
    let lower_keyword = keyword.to_lowercase();

    if let Some(pos) = lower_text.find(&lower_keyword) {
        let char_pos = full_text.char_indices().position(|(i, _)| i >= pos).unwrap_or(0);
        let start = char_pos.saturating_sub(context_chars / 2);
        let end = (char_pos + keyword.chars().count() + context_chars / 2).min(full_text.chars().count());

        let snippet: String = full_text.chars().skip(start).take(end - start).collect();
        format!("……{}……", snippet.trim())
    } else {
        String::new()
    }
}
