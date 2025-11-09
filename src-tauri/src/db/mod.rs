use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Database {
    _path: PathBuf,
    conn: parking_lot::Mutex<Connection>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ItemDto {
    pub id: i64,
    pub created_at: i64,
    pub kind: String, // "text" | "image" | "file"
    pub size: i64,
    pub sha256_hex: String,
    pub file_path: Option<String>,
    pub is_pinned: bool,
    // note: encrypted blobs are not exposed to UI directly
}

#[derive(Debug, Clone)]
pub struct NewItem {
    pub kind: String,
    pub size: i64,
    pub sha256: Vec<u8>,
    pub file_path: Option<String>,
    pub content_blob: Option<Vec<u8>>, // nonce||ciphertext
    pub preview_blob: Option<Vec<u8>>, // nonce||ciphertext
    pub rtf_blob: Option<Vec<u8>>,     // nonce||ciphertext
}

impl Database {
    pub fn new(app_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&app_dir)?;
        let db_path = app_dir.join("cliper.sqlite");
        let conn = Connection::open(&db_path)?;
        conn.pragma_update(None, "journal_mode", &"WAL")?;
        Ok(Self {
            _path: db_path,
            conn: parking_lot::Mutex::new(conn),
        })
    }

    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS items (
              id INTEGER PRIMARY KEY,
              created_at INTEGER NOT NULL,
              kind TEXT NOT NULL,
              size INTEGER NOT NULL,
              sha256 BLOB NOT NULL,
              file_path TEXT,
              is_pinned INTEGER NOT NULL DEFAULT 0,
              content_blob BLOB,
              preview_blob BLOB,
              rtf_blob BLOB
            );
            CREATE INDEX IF NOT EXISTS idx_items_created ON items(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_items_kind ON items(kind);
            "#,
        )?;
        Ok(())
    }

    pub fn insert_item(&self, item: NewItem) -> Result<i64> {
        // Deduplicate by sha256 + kind + file_path
        let maybe = self.find_by_hash_kind_path(&item.sha256, &item.kind, item.file_path.as_deref())?;
        if let Some(id) = maybe {
            return Ok(id);
        }

        let ts = now_millis();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO items (created_at, kind, size, sha256, file_path, is_pinned, content_blob, preview_blob, rtf_blob)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)",
            params![
                ts,
                item.kind,
                item.size,
                item.sha256,
                item.file_path,
                item.content_blob,
                item.preview_blob,
                item.rtf_blob
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn find_by_hash_kind_path(
        &self,
        sha256: &[u8],
        kind: &str,
        file_path: Option<&str>,
    ) -> Result<Option<i64>> {
        let conn = self.conn.lock();
        let id: Option<i64> = conn
            .query_row(
                "SELECT id FROM items WHERE sha256 = ?1 AND kind = ?2 AND IFNULL(file_path,'') = IFNULL(?3,'') ORDER BY id DESC LIMIT 1",
                params![sha256, kind, file_path],
                |row| row.get(0),
            )
            .optional()?;
        Ok(id)
    }

    pub fn list_recent(&self, limit: u32) -> Result<Vec<ItemDto>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, created_at, kind, size, sha256, file_path, is_pinned FROM items ORDER BY is_pinned DESC, created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let sha: Vec<u8> = row.get(4)?;
            Ok(ItemDto {
                id: row.get(0)?,
                created_at: row.get(1)?,
                kind: row.get::<_, String>(2)?,
                size: row.get(3)?,
                sha256_hex: hex::encode(sha),
                file_path: row.get(5)?,
                is_pinned: row.get::<_, i64>(6)? != 0,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_item_raw(&self, id: i64) -> Result<(String, Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>, Option<String>)> {
        let conn = self.conn.lock();
        let row: (String, Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<u8>>, Option<String>) = conn
            .query_row(
                "SELECT kind, content_blob, preview_blob, rtf_blob, file_path FROM items WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )?;
        Ok(row)
    }

    pub fn pin_item(&self, id: i64, pin: bool) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE items SET is_pinned = ?2 WHERE id = ?1",
            params![id, if pin { 1 } else { 0 }],
        )?;
        Ok(())
    }

    pub fn delete_item(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM items WHERE id = ?1", params![id])?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn clear_all(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM items", [])?;
        Ok(())
    }

    pub fn compute_sha256(data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }
}

pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
