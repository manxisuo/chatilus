use std::fs::{self, File};
use std::io::{copy, Read};
use std::path::{Path, PathBuf};

use crate::domain::models::ImportProgress;
use crate::infrastructure::importers::chatgpt::find_conversation_files;
use std::sync::Arc;

const CHATGPT_IMPORTER_VERSION: &str = "0.1.0";

pub struct ResolvedImportPath {
    pub export_dir: PathBuf,
    pub cleanup: Option<TempExtractDir>,
    pub export_label: String,
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

pub fn resolve_import_path(
    input_path: &Path,
    on_progress: Option<Arc<dyn Fn(ImportProgress) + Send + Sync>>,
) -> Result<ResolvedImportPath, String> {
    if input_path.is_dir() {
        let export_dir = find_chatgpt_export_root(input_path)?;
        let export_label = export_label_from_path(input_path);
        return Ok(ResolvedImportPath {
            export_dir,
            cleanup: None,
            export_label,
        });
    }

    if is_zip_file(input_path) {
        report(on_progress.as_ref(), "extracting", 0, 1, 0.02);
        let cleanup = TempExtractDir {
            path: create_extract_dir(input_path)?,
        };
        extract_zip(input_path, cleanup.path())?;
        report(on_progress.as_ref(), "extracting", 1, 1, 0.18);
        extract_nested_zips(cleanup.path(), on_progress.clone())?;
        let export_dir = find_chatgpt_export_root(cleanup.path())?;
        let export_label = export_label_from_path(input_path);
        return Ok(ResolvedImportPath {
            export_dir,
            cleanup: Some(cleanup),
            export_label,
        });
    }

    Err(format!(
        "路径不存在或不是支持的导入格式（目录 / zip）: {}",
        input_path.display()
    ))
}

pub fn chatgpt_importer_version() -> &'static str {
    CHATGPT_IMPORTER_VERSION
}

fn export_label_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("chatgpt-export")
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

pub fn find_chatgpt_export_root(search_root: &Path) -> Result<PathBuf, String> {
    if find_conversation_files(search_root).is_ok() {
        return Ok(search_root.to_path_buf());
    }

    let mut queue = vec![search_root.to_path_buf()];
    while let Some(dir) = queue.pop() {
        let entries = fs::read_dir(&dir).map_err(|e| format!("无法读取目录 {}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("读取目录项失败: {e}"))?;
            let path = entry.path();
            if path.is_dir() {
                if find_conversation_files(&path).is_ok() {
                    return Ok(path);
                }
                queue.push(path);
            }
        }
    }

    Err(format!(
        "在 {} 中未找到 ChatGPT 导出（conversations*.json）",
        search_root.display()
    ))
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
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn write_test_zip(path: &Path, json_name: &str, json_body: &str) {
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
    }
}
