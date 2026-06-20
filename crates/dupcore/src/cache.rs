//! On-disk hash/fingerprint cache to speed up re-runs.
//!
//! Keyed by `(path, size, mtime)`. When any of size or mtime changes the cached
//! value is stale and ignored (a fresh hash overwrites it). This is a pure
//! performance optimisation — correctness never depends on the cache.

use crate::error::{DupError, Result};
use rusqlite::Connection;
use std::path::Path;

pub struct HashCache {
    conn: Connection,
}

impl HashCache {
    /// Open (creating if needed) a cache database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| DupError::Cache(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hashes (
                 path  TEXT NOT NULL,
                 algo  INTEGER NOT NULL,
                 size  INTEGER NOT NULL,
                 mtime INTEGER NOT NULL,
                 hash  TEXT NOT NULL,
                 PRIMARY KEY (path, algo)
             );
             CREATE TABLE IF NOT EXISTS fingerprints (
                 path  TEXT PRIMARY KEY,
                 size  INTEGER NOT NULL,
                 mtime INTEGER NOT NULL,
                 fp    TEXT NOT NULL
             );",
        )
        .map_err(|e| DupError::Cache(e.to_string()))?;
        Ok(Self { conn })
    }

    /// In-memory cache (for tests).
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| DupError::Cache(e.to_string()))?;
        conn.execute_batch(
            "CREATE TABLE hashes (path TEXT, algo INTEGER, size INTEGER, mtime INTEGER, hash TEXT, PRIMARY KEY(path,algo));
             CREATE TABLE fingerprints (path TEXT PRIMARY KEY, size INTEGER, mtime INTEGER, fp TEXT);",
        )
        .map_err(|e| DupError::Cache(e.to_string()))?;
        Ok(Self { conn })
    }

    /// Fetch a cached full hash if size+mtime still match.
    pub fn get_hash(&self, path: &str, algo: u8, size: u64, mtime: i64) -> Option<String> {
        self.conn
            .query_row(
                "SELECT hash FROM hashes WHERE path=?1 AND algo=?2 AND size=?3 AND mtime=?4",
                rusqlite::params![path, algo, size as i64, mtime],
                |row| row.get::<_, String>(0),
            )
            .ok()
    }

    pub fn put_hash(&self, path: &str, algo: u8, size: u64, mtime: i64, hash: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO hashes(path,algo,size,mtime,hash) VALUES (?1,?2,?3,?4,?5)",
                rusqlite::params![path, algo, size as i64, mtime, hash],
            )
            .map_err(|e| DupError::Cache(e.to_string()))?;
        Ok(())
    }

    /// Fetch a cached video fingerprint if size+mtime still match.
    pub fn get_fingerprint(&self, path: &str, size: u64, mtime: i64) -> Option<String> {
        self.conn
            .query_row(
                "SELECT fp FROM fingerprints WHERE path=?1 AND size=?2 AND mtime=?3",
                rusqlite::params![path, size as i64, mtime],
                |row| row.get::<_, String>(0),
            )
            .ok()
    }

    pub fn put_fingerprint(&self, path: &str, size: u64, mtime: i64, fp: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO fingerprints(path,size,mtime,fp) VALUES (?1,?2,?3,?4)",
                rusqlite::params![path, size as i64, mtime, fp],
            )
            .map_err(|e| DupError::Cache(e.to_string()))?;
        Ok(())
    }
}
