//! Library (section 7): the unified index of downloaded and received items, queries, delete,
//! move, duplicate detection (size plus hash) and startup reconciliation.

use std::collections::HashMap;
use std::path::PathBuf;

use dg_types::ErrorCode;
use serde_json::{json, Value};

use crate::db::ItemRow;
use crate::engine::{err, internal, Engine, Res};
use crate::util::{now_s, when_label};

fn item_json(it: &ItemRow) -> Value {
    let source = match (it.origin.as_str(), it.source_url.as_deref(), it.source_device.as_deref()) {
        ("download", Some(url), _) => format!("Downloaded from {}", crate::util::host_of(url).unwrap_or_else(|| "a link".into())),
        ("received", _, Some(dev)) => format!("Received from {dev}"),
        ("download", _, _) => "Downloaded".to_string(),
        _ => "Imported".to_string(),
    };
    json!({
        "id": it.id,
        "name": it.display_name,
        "kind": it.kind,
        "size": it.size,
        "origin": it.origin,
        "source": source,
        "when": when_label(it.created_at),
        "path": it.path,
        "sha256": it.sha256,
        "mime": it.mime,
        "sourceUrl": it.source_url,
        "sourceDevice": it.source_device,
        "createdAt": it.created_at,
    })
}

/// `library.query {filter, search}`. Filters match the UI chips: all, videos, music, images,
/// documents, archives, received.
pub fn query(engine: &Engine, params: &Value) -> Res<Value> {
    let all = engine.db.items_all().map_err(internal)?;
    let total = all.len();
    let filter = params.get("filter").and_then(Value::as_str).unwrap_or("all");
    let search = params.get("search").and_then(Value::as_str).unwrap_or("").trim().to_lowercase();
    let items: Vec<Value> = all
        .iter()
        .filter(|it| match filter {
            "all" | "" => true,
            "received" => it.origin == "received",
            "videos" => it.kind == "video",
            "music" => it.kind == "audio",
            "images" => it.kind == "image",
            "documents" => it.kind == "document",
            "archives" => it.kind == "archive",
            "apps" => it.kind == "app",
            _ => false,
        })
        .filter(|it| search.is_empty() || it.display_name.to_lowercase().contains(&search))
        .map(item_json)
        .collect();
    Ok(json!({ "items": items, "total": total }))
}

pub fn get(engine: &Engine, params: &Value) -> Res<Value> {
    let id = params.get("id").and_then(Value::as_str).ok_or_else(|| err(ErrorCode::InvalidParams, "id is required"))?;
    let it = engine.db.item_get(id).map_err(internal)?;
    Ok(json!({ "item": it.as_ref().map(item_json) }))
}

/// `library.delete {ids}`: removes the rows and the files they point to.
pub fn delete(engine: &Engine, params: &Value) -> Res<Value> {
    let ids: Vec<String> = params
        .get("ids")
        .and_then(Value::as_array)
        .ok_or_else(|| err(ErrorCode::InvalidParams, "ids is required"))?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let mut deleted = 0;
    for id in &ids {
        if let Some(it) = engine.db.item_get(id).map_err(internal)? {
            let _ = std::fs::remove_file(&it.path);
            engine.db.item_delete(id).map_err(internal)?;
            deleted += 1;
        }
    }
    engine.emit("library.changed", json!({ "reason": "deleted", "ids": ids }));
    Ok(json!({ "deleted": deleted }))
}

/// `library.move {id, folder}`: moves one item into a folder under the download root.
pub fn move_item(engine: &Engine, params: &Value) -> Res<Value> {
    let id = params.get("id").and_then(Value::as_str).ok_or_else(|| err(ErrorCode::InvalidParams, "id is required"))?;
    let folder = params.get("folder").and_then(Value::as_str).ok_or_else(|| err(ErrorCode::InvalidParams, "folder is required"))?;
    if folder.contains("..") || folder.starts_with('/') || folder.contains('\\') {
        return Err(err(ErrorCode::InvalidParams, "folder must be a relative name"));
    }
    let it = engine.db.item_get(id).map_err(internal)?.ok_or_else(|| err(ErrorCode::UnknownItem, "no such item"))?;
    let dir = PathBuf::from(engine.settings().download_folder).join(folder);
    std::fs::create_dir_all(&dir).map_err(internal)?;
    let target = crate::util::unique_path(&dir, &it.display_name);
    std::fs::rename(&it.path, &target).or_else(|_| std::fs::copy(&it.path, &target).and_then(|_| std::fs::remove_file(&it.path))).map_err(internal)?;
    engine
        .db
        .item_set_path(id, &target.display().to_string(), &target.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
        .map_err(internal)?;
    engine.emit("library.changed", json!({ "reason": "moved", "ids": [id] }));
    Ok(json!({ "ok": true, "path": target.display().to_string() }))
}

/// `library.duplicates`: groups by size, then confirms with SHA-256 (section 7.4).
pub fn duplicates(engine: &Engine) -> Res<Value> {
    let items = engine.db.items_all().map_err(internal)?;
    let mut by_size: HashMap<i64, Vec<ItemRow>> = HashMap::new();
    for it in items {
        if it.size > 0 {
            by_size.entry(it.size).or_default().push(it);
        }
    }
    let mut groups = Vec::new();
    for (_, mut same) in by_size {
        if same.len() < 2 {
            continue;
        }
        for it in same.iter_mut() {
            if it.sha256.is_none() {
                if let Ok(h) = crate::transfer::sha256_file(std::path::Path::new(&it.path)) {
                    let _ = engine.db.item_set_sha(&it.id, &h);
                    it.sha256 = Some(h);
                }
            }
        }
        let mut by_hash: HashMap<String, Vec<&ItemRow>> = HashMap::new();
        for it in &same {
            if let Some(h) = &it.sha256 {
                by_hash.entry(h.clone()).or_default().push(it);
            }
        }
        for (hash, members) in by_hash {
            if members.len() > 1 {
                groups.push(json!({ "sha256": hash, "items": members.iter().map(|m| item_json(m)).collect::<Vec<_>>() }));
            }
        }
    }
    Ok(json!({ "groups": groups }))
}

/// Adds a finished file that the engine created outside the normal flows (used by tests).
pub fn add_imported(engine: &Engine, path: &std::path::Path, origin: &str) -> Res<String> {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let meta = std::fs::metadata(path).map_err(internal)?;
    let id = uuid::Uuid::new_v4().to_string();
    engine
        .db
        .item_insert(&ItemRow {
            id: id.clone(),
            path: path.display().to_string(),
            display_name: name.clone(),
            mime: Some(crate::util::mime_from_name(&name).to_string()),
            kind: crate::util::kind_from_name(&name).to_string(),
            size: meta.len() as i64,
            sha256: None,
            origin: origin.to_string(),
            source_url: None,
            source_device: None,
            created_at: now_s(),
        })
        .map_err(internal)?;
    engine.emit("library.changed", json!({ "reason": "imported", "ids": [id] }));
    Ok(id)
}
