use std::fs::{self, File};
use std::io::{copy, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::domain::models::ImportProgress;
use crate::infrastructure::importers::{default_importer_registry, ImporterRegistry};

pub struct ResolvedImportPath {
    pub export_dir: PathBuf,
    pub cleanup: Option<TempExtractDir>,
    pub export_label: String,
    pub importer_id: String,
    pub importer_display_name: String,
}

pub struct TempExtractDir {
    path: PathBuf,
}

impl TempExtractDir {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempExtractDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn resolve_import_path_for_importer(
    input_path: &Path,
    importer_id: &str,
    registry: &ImporterRegistry,
    on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
) -> Result<ResolvedImportPath, String> {
    let importer = registry
        .by_id(importer_id)
        .ok_or_else(|| format!("未知数据源: {importer_id}"))?;

    if input_path.is_dir() {
        let (export_dir, detect) = registry.find_export_root_for_importer(input_path, importer_id)?;
        let export_label = export_label_from_path(input_path);
        return Ok(ResolvedImportPath {
            export_dir,
            cleanup: None,
            export_label,
            importer_id: detect.importer_id,
            importer_display_name: detect.display_name,
        });
    }

    if input_path.is_file() {
        if is_zip_file(input_path) {
            if importer_id != "chatgpt" {
                return Err(format!(
                    "{} 不支持 ZIP 文件，请选择目录或其它文件类型",
                    importer.display_name()
                ));
            }
            report(on_progress.as_ref(), "extracting", 0, 1, 0.02);
            let cleanup = TempExtractDir {
                path: create_extract_dir(input_path)?,
            };
            extract_zip(input_path, cleanup.path())?;
            report(on_progress.as_ref(), "extracting", 1, 1, 0.18);
            extract_nested_zips(cleanup.path(), on_progress.clone())?;
            let (export_dir, detect) =
                registry.find_export_root_for_importer(cleanup.path(), importer_id)?;
            let export_label = export_label_from_path(input_path);
            return Ok(ResolvedImportPath {
                export_dir,
                cleanup: Some(cleanup),
                export_label,
                importer_id: detect.importer_id,
                importer_display_name: detect.display_name,
            });
        }

        let detect = registry.detect_with_importer(input_path, importer_id)?;
        let export_label = export_label_from_path(input_path);
        return Ok(ResolvedImportPath {
            export_dir: input_path.to_path_buf(),
            cleanup: None,
            export_label,
            importer_id: detect.importer_id,
            importer_display_name: detect.display_name,
        });
    }

    Err(format!(
        "路径不存在或不是 {} 支持的导入格式: {}",
        importer.display_name(),
        input_path.display()
    ))
}

pub fn resolve_import_path(
    input_path: &Path,
    on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
) -> Result<ResolvedImportPath, String> {
    resolve_import_path_with_registry(input_path, &default_importer_registry(), on_progress)
}

pub fn resolve_import_path_with_registry(
    input_path: &Path,
    registry: &ImporterRegistry,
    on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
) -> Result<ResolvedImportPath, String> {
    if input_path.is_dir() {
        let (export_dir, detect) = registry.find_export_root(input_path)?;
        let export_label = export_label_from_path(input_path);
        return Ok(ResolvedImportPath {
            export_dir,
            cleanup: None,
            export_label,
            importer_id: detect.importer_id,
            importer_display_name: detect.display_name,
        });
    }

    if input_path.is_file() {
        if is_zip_file(input_path) {
            report(on_progress.as_ref(), "extracting", 0, 1, 0.02);
            let cleanup = TempExtractDir {
                path: create_extract_dir(input_path)?,
            };
            extract_zip(input_path, cleanup.path())?;
            report(on_progress.as_ref(), "extracting", 1, 1, 0.18);
            extract_nested_zips(cleanup.path(), on_progress.clone())?;
            let (export_dir, detect) = registry.find_export_root(cleanup.path())?;
            let export_label = export_label_from_path(input_path);
            return Ok(ResolvedImportPath {
                export_dir,
                cleanup: Some(cleanup),
                export_label,
                importer_id: detect.importer_id,
                importer_display_name: detect.display_name,
            });
        }

        if let Ok(detect) = registry.detect(input_path) {
            let export_label = export_label_from_path(input_path);
            return Ok(ResolvedImportPath {
                export_dir: input_path.to_path_buf(),
                cleanup: None,
                export_label,
                importer_id: detect.importer_id,
                importer_display_name: detect.display_name,
            });
        }
    }

    Err(format!(
        "路径不存在或不是支持的导入格式（目录 / zip / Cursor state.vscdb / Codex state.sqlite）: {}",
        input_path.display()
    ))
}

fn export_label_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("export")
        .to_string()
}

fn is_zip_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        return true;
    }

    let mut header = [0_u8; 4];
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    file.read_exact(&mut header).is_ok() && header[0] == 0x50 && header[1] == 0x4B
}

fn create_extract_dir(zip_path: &Path) -> Result<PathBuf, String> {
    let base = std::env::temp_dir().join("chatlens-import");
    fs::create_dir_all(&base).map_err(|e| format!("无法创建临时目录: {e}"))?;
    let dir = base.join(format!(
        "{}-{}",
        zip_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("export"),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建解压目录: {e}"))?;
    Ok(dir)
}

fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dest_dir).map_err(|e| format!("无法创建解压目录: {e}"))?;
    let file =
        File::open(zip_path).map_err(|e| format!("无法打开 zip 文件 {}: {e}", zip_path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("无法读取 zip 文件: {e}"))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("读取 zip 条目失败: {e}"))?;
        let entry_path = match entry.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };

        if entry.name().ends_with('/') {
            fs::create_dir_all(&entry_path)
                .map_err(|e| format!("创建目录 {} 失败: {e}", entry_path.display()))?;
            continue;
        }

        if let Some(parent) = entry_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录 {} 失败: {e}", parent.display()))?;
        }

        let mut outfile = File::create(&entry_path)
            .map_err(|e| format!("创建文件 {} 失败: {e}", entry_path.display()))?;
        copy(&mut entry, &mut outfile).map_err(|e| format!("解压文件失败: {e}"))?;
    }

    Ok(())
}

fn extract_nested_zips(
    root: &Path,
    on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
) -> Result<(), String> {
    let mut pending = collect_zip_files(root)?;
    let mut processed = 0usize;
    let initial_total = pending.len().max(1);

    while let Some(zip_path) = pending.pop() {
        let dest_dir = zip_path
            .parent()
            .ok_or_else(|| format!("无法确定 {} 的父目录", zip_path.display()))?
            .join(
                zip_path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("nested"),
            );
        extract_zip(&zip_path, &dest_dir)?;
        pending.extend(collect_zip_files(&dest_dir)?);
        processed += 1;
        let progress = 0.18 + (processed as f64 / initial_total as f64) * 0.02;
        report(
            on_progress.as_ref(),
            "extracting",
            processed,
            initial_total,
            progress.min(0.2),
        );
    }

    Ok(())
}

fn collect_zip_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_zip_files_recursively(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_zip_files_recursively(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("无法读取目录 {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_zip_files_recursively(&path, files)?;
        } else if is_zip_file(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn report(
    on_progress: Option<&Arc<dyn Fn(ImportProgress) + Send + Sync>>,
    phase: &str,
    processed: usize,
    total: usize,
    progress: f64,
) {
    if let Some(callback) = on_progress {
        callback(ImportProgress {
            phase: phase.to_string(),
            progress,
            processed,
            total: total.max(1),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::importers::chatgpt::find_conversation_files;

    fn write_test_zip(path: &Path, json_name: &str, json_body: &str) {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        use zip::ZipWriter;

        let file = File::create(path).expect("create zip");
        let mut zip = ZipWriter::new(file);
        zip.start_file(
            json_name,
            SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .expect("start file");
        zip.write_all(json_body.as_bytes()).expect("write json");
        zip.finish().expect("finish zip");
    }

    #[test]
    fn extracts_zip_and_finds_export_root() {
        let base = std::env::temp_dir().join(format!(
            "chatlens-zip-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("mkdir");

        let zip_path = base.join("sample-export.zip");
        write_test_zip(
            &zip_path,
            "conversations-000.json",
            r#"[{"id":"c1","title":"Test","mapping":{},"current_node":null}]"#,
        );

        let resolved = resolve_import_path(&zip_path, None).expect("resolve");
        assert!(find_conversation_files(&resolved.export_dir).is_ok());
        assert_eq!(resolved.export_label, "sample-export");
        assert_eq!(resolved.importer_id, "chatgpt");
    }

    #[test]
    fn resolves_cursor_vscdb_file() {
        use crate::infrastructure::importers::cursor::resolve_cursor_db_path;

        let db = std::env::var("APPDATA")
            .map(|appdata| {
                std::path::PathBuf::from(appdata)
                    .join("Cursor")
                    .join("User")
                    .join("globalStorage")
                    .join("state.vscdb")
            })
            .unwrap_or_default();
        if !db.is_file() {
            return;
        }

        let resolved = resolve_import_path(&db, None).expect("resolve");
        assert_eq!(resolved.importer_id, "cursor");
        assert_eq!(resolved.export_dir, db);
        assert_eq!(resolve_cursor_db_path(&resolved.export_dir).as_deref(), Some(db.as_path()));
    }
}
