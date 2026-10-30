//! Port of `SPDBConnection`: a thin adapter between models and the
//! [`Database`], so models never touch the connection directly.

use rusqlite::{Connection, Params, Row};

use crate::database::Database;

pub struct SPDBConnection<'a> {
    db: &'a Database,
}

impl<'a> SPDBConnection<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn handle(&self) -> &Connection {
        self.db.handle()
    }

    /// Executes a statement with no result rows. Returns true on success.
    pub fn execute<P: Params>(&self, sql: &str, params: P) -> bool {
        match self.handle().execute(sql, params) {
            Ok(_) => true,
            Err(e) => {
                log::warn!("SPDBConnection::execute failed: {e}");
                false
            }
        }
    }

    /// Executes a one-shot batch of statements.
    pub fn exec(&self, sql: &str) -> Result<(), String> {
        self.handle().execute_batch(sql).map_err(|e| format!("SPDBConnection::exec failed: {e}"))
    }

    /// Runs a query and maps every row.
    pub fn query<T, P: Params>(
        &self,
        sql: &str,
        params: P,
        from_row: impl FnMut(&Row) -> rusqlite::Result<T>,
    ) -> Vec<T> {
        let mut stmt = match self.handle().prepare(sql) {
            Ok(stmt) => stmt,
            Err(e) => {
                log::warn!("SPDBConnection::prepare failed: {e}");
                return Vec::new();
            }
        };
        stmt.query_map(params, from_row).map(|rows| rows.flatten().collect()).unwrap_or_default()
    }

    /// Runs a query and maps the first row.
    pub fn query_one<T, P: Params>(
        &self,
        sql: &str,
        params: P,
        from_row: impl FnMut(&Row) -> rusqlite::Result<T>,
    ) -> Option<T> {
        self.query(sql, params, from_row).into_iter().next()
    }

    pub fn last_insert_rowid(&self) -> i64 {
        self.handle().last_insert_rowid()
    }
}

/// Column text as String ("" for NULL).
pub fn text(row: &Row, col: usize) -> String {
    row.get::<_, Option<String>>(col).ok().flatten().unwrap_or_default()
}

/// Empty string → SQL NULL.
pub fn opt_text(v: &str) -> Option<&str> {
    (!v.is_empty()).then_some(v)
}
