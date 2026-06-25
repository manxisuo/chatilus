use chatlens_lib::infrastructure::importers::{CodexImporter, default_importer_registry};
use chatlens_lib::domain::ports::{Importer, ImportInput, ImportOptions};
use chatlens_lib::infrastructure::importers::codex::resolve::default_codex_home;

fn main() {
    let home = default_codex_home();
    if !home.is_dir() { eprintln!("no codex"); return; }
    let importer = CodexImporter::new();
    let result = importer.import(&ImportInput { path: home }, &ImportOptions::default()).unwrap();
    let mut msgs = 0usize;
    let mut atts = 0usize;
    let mut with_path = 0usize;
    for c in &result.package.conversations {
        for m in &c.messages {
            msgs += 1;
            atts += m.attachments.len();
            with_path += m.attachments.iter().filter(|a| a.path.is_some()).count();
        }
    }
    println!("conversations={} messages={} attachments={} with_path={} media_indexed={}", 
        result.package.conversations.len(), msgs, atts, with_path, result.media_files_indexed);
}
