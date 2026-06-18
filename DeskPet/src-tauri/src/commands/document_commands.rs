use tauri::State;

use crate::commands::chat_commands::AppState;
use crate::data::database::DocumentKnowledgeRow;
use crate::services::extract_service;
use crate::services::summarize_service;

#[tauri::command]
pub async fn import_document(
    file_path: String,
    state: State<'_, AppState>,
) -> Result<DocumentKnowledgeRow, String> {
    let extracted = extract_service::extract_text(&file_path)?;

    let id = uuid::Uuid::new_v4().to_string();

    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        let db = db_guard.as_ref().ok_or("数据库未初始化")?;
        db.save_document(
            &id,
            &extracted.title,
            &file_path,
            &extracted.file_type,
            extracted.word_count,
            &extracted.full_text,
        )?;
        db.update_document_status(&id, "summarizing", None)?;
    }

    let ai_config = {
        let locked = state.ai_config.lock().map_err(|e| e.to_string())?;
        locked.as_ref().ok_or("AI 接口未配置")?.clone()
    };

    let summary_result = summarize_service::generate_summary(
        &ai_config,
        &extracted.title,
        &extracted.full_text,
    ).await;

    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        let db = db_guard.as_ref().ok_or("数据库未初始化")?;
        match summary_result {
            Ok(summary_json) => {
                db.update_document_summary(&id, &summary_json)?;
            }
            Err(err) => {
                db.update_document_status(&id, "error", Some(&err))?;
            }
        }
    }

    get_document_by_id_inner(&state, &id)
}

#[tauri::command]
pub fn get_document_list(
    state: State<'_, AppState>,
) -> Result<Vec<DocumentKnowledgeRow>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;
    db.get_document_list()
}

#[tauri::command]
pub fn get_document_detail(
    id: String,
    state: State<'_, AppState>,
) -> Result<DocumentKnowledgeRow, String> {
    get_document_by_id_inner(&state, &id)
}

#[tauri::command]
pub fn delete_document(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;
    db.delete_document(&id)
}

#[tauri::command]
pub fn search_document_context(
    doc_id: String,
    keyword: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;
    let doc = db.get_document_by_id(&doc_id)?;

    let full_text = doc.full_text.as_deref().unwrap_or("");
    Ok(extract_service::search_in_text(full_text, &keyword, 2000))
}

fn get_document_by_id_inner(
    state: &State<'_, AppState>,
    id: &str,
) -> Result<DocumentKnowledgeRow, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("数据库未初始化")?;
    db.get_document_by_id(id)
}
