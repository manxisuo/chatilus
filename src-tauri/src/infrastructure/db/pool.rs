use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::error::AppResult;

use super::Database;

/// Cap on idle connections kept in the pool. Concurrent commands beyond this
/// open short-lived connections instead of blocking each other.
const DEFAULT_MAX_IDLE: usize = 8;

struct PoolInner {
    path: PathBuf,
    idle: Mutex<Vec<Database>>,
    max_idle: usize,
}

/// SQLite connection pool. WAL allows concurrent readers against the same
/// library file; each checkout gets its own `rusqlite::Connection`.
pub struct DbPool {
    inner: Arc<PoolInner>,
}

/// RAII guard: derefs to `Database`, returns the connection on drop.
pub struct PooledDatabase {
    db: Option<Database>,
    pool: Arc<PoolInner>,
}

impl DbPool {
    pub fn open(path: &Path) -> AppResult<Self> {
        // Warm the pool and run schema migrations once.
        let first = Database::open(path)?;
        Ok(Self {
            inner: Arc::new(PoolInner {
                path: path.to_path_buf(),
                idle: Mutex::new(vec![first]),
                max_idle: DEFAULT_MAX_IDLE,
            }),
        })
    }

    pub fn get(&self) -> AppResult<PooledDatabase> {
        let db = {
            let mut idle = self
                .inner
                .idle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            idle.pop()
        };

        let db = match db {
            Some(db) => db,
            None => Database::open(&self.inner.path)?,
        };

        Ok(PooledDatabase {
            db: Some(db),
            pool: self.inner.clone(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.inner.path
    }
}

impl Deref for PooledDatabase {
    type Target = Database;

    fn deref(&self) -> &Self::Target {
        self.db.as_ref().expect("pooled database already taken")
    }
}

impl DerefMut for PooledDatabase {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.db.as_mut().expect("pooled database already taken")
    }
}

impl Drop for PooledDatabase {
    fn drop(&mut self) {
        if let Some(db) = self.db.take() {
            let mut idle = self
                .pool
                .idle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if idle.len() < self.pool.max_idle {
                idle.push(db);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ports::ConversationRepository;
    use crate::domain::models::DataSource;

    #[test]
    fn pool_checkouts_are_independent() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let path = std::env::temp_dir().join(format!("chatlens-pool-{stamp}.db"));
        let _ = std::fs::remove_file(&path);

        let pool = DbPool::open(&path).expect("open pool");
        let a = pool.get().expect("checkout a");
        let b = pool.get().expect("checkout b");

        // Two live connections against the same file.
        drop(a);
        let c = pool.get().expect("checkout c");
        drop(b);
        drop(c);

        // Reuse: another checkout after returns.
        let d = pool.get().expect("checkout d");
        drop(d);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pool_writes_and_reads_conversation() {
        use crate::domain::ports::{ImportedConversation, ImportedMessage};

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let path = std::env::temp_dir().join(format!("chatlens-pool-write-{stamp}.db"));
        let _ = std::fs::remove_file(&path);

        let pool = DbPool::open(&path).expect("open pool");
        {
            let mut db = pool.get().expect("write conn");
            ConversationRepository::save_many(
                &mut *db,
                &[ImportedConversation {
                    id: "c1".to_string(),
                    title: "Pool".to_string(),
                    create_time: Some(1.0),
                    update_time: Some(2.0),
                    model: None,
                    messages: vec![ImportedMessage {
                        id: "m1".to_string(),
                        role: "user".to_string(),
                        content: "hello".to_string(),
                        create_time: Some(1.0),
                        raw_json: "{}".to_string(),
                        attachments: Vec::new(),
                    }],
                    source_contexts: Vec::new(),
                }],
                "/tmp",
                DataSource::ChatGpt,
                0,
            )
            .expect("save_many");
        }

        {
            let db = pool.get().expect("read conn");
            let summary = ConversationRepository::get_summary(&*db, "chatgpt::c1")
                .expect("get")
                .expect("summary");
            assert_eq!(summary.title, "Pool");
        }

        let _ = std::fs::remove_file(&path);
    }
}
