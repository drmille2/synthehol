use rusqlite::params;
use tokio_rusqlite::Connection;
use tracing::{debug, error, instrument};

use crate::monitor::MonitorResult;

#[derive(Debug)]
pub struct SynthDb {
    pub db: Option<&'static tokio_rusqlite::Connection>,
}

impl SynthDb {
    pub async fn new(path: Option<&str>) -> Result<Self, tokio_rusqlite::Error> {
        let db = match path {
            Some(path) => Connection::open(path).await?,
            None => Connection::open_in_memory().await?,
        };

        let db = Box::leak(Box::new(db));

        Ok(SynthDb { db: Some(db) })
    }

    #[instrument]
    pub async fn initialize_db(&self) -> Result<(), tokio_rusqlite::Error> {
        // create results table if it doesn't exist
        debug!("setting sqlite pragmas...");
        if let Some(db) = self.db {
            db.call(|db| {
                db.execute("PRAGMA cache_size = -4096", [])
                    .map_err(|e| e.into())
            })
            .await
            .unwrap_or_else(|e| {
                error!("failed to set pragmas ({})", e);
                0
            });
        }

        debug!("attempting to create results table...");
        if let Some(db) = self.db {
            db.call(|db| {
                db.execute(
                    "CREATE TABLE IF NOT EXISTS results (
                        id    INTEGER PRIMARY KEY,
                        monitor_name  TEXT NOT NULL,
                        level_name TEXT NOT NULL,
                        start_time INTEGER NOT NULL,
                        target_name   TEXT NOT NULL,
                        args TEXT,
                        stdout TEXT,
                        stderr TEXT,
                        duration INTEGER,
                        status INTEGER
                    )",
                    [],
                )
                .map_err(|e| e.into())
            })
            .await?;
        }

        // create monitor_state table if it doesn't exist
        debug!("attempting to create monitor_state table...");
        if let Some(db) = self.db {
            db.call(|db| {
                db.execute(
                    "CREATE TABLE IF NOT EXISTS monitor_state (
                        id INTEGER PRIMARY KEY,
                        name  TEXT UNIQUE,
                        level_index INTEGER NOT NULL,
                        failure_tally INTEGER NOT NULL,
                        success_tally INTEGER NOT NULL
                    )",
                    [],
                )
                .map_err(|e| e.into())
            })
            .await?;
        }

        // create reporter_state table if it doesn't exist
        debug!("attempting to create reporter_state table...");
        if let Some(db) = self.db {
            db.call(|db| {
                db.execute(
                    "CREATE TABLE IF NOT EXISTS reporter_state (
                        id INTEGER PRIMARY KEY,
                        name TEXT,
                        monitor_name TEXT,
                        state BLOB
                    )",
                    [],
                )
                .map_err(|e| e.into())
            })
            .await?;
        }

        // add unique reporter state index
        if let Some(db) = self.db {
            db.call(|db| {
                db.execute(
                    "CREATE UNIQUE INDEX idx_reporter_state_name_monitor_name 
                        ON reporter_state (name, monitor_name);",
                    [],
                )
                .map_err(|e| e.into())
            })
            .await?;
        }
        Ok(())
    }

    #[instrument]
    pub async fn record_result(&self, res: MonitorResult) -> Result<(), tokio_rusqlite::Error> {
        let MonitorResult {
            name: monitor_name,
            level_name,
            start_time,
            target: target_name,
            args,
            stdout,
            stderr,
            duration,
            status,
        } = res;
        if let Some(db) = self.db {
            db.call(move |db| {
                db.execute(
                    "INSERT INTO
                        results (
                            monitor_name, 
                            level_name,
                            start_time,
                            target_name,
                            args,
                            stdout,
                            stderr,
                            duration,
                            status
                        )
                    VALUES (
                            ?1,
                            ?2,
                            ?3,
                            ?4,
                            ?5,
                            ?6,
                            ?7,
                            ?8,
                            ?9
                        )
                    ",
                    params![
                        monitor_name,
                        level_name,
                        start_time,
                        target_name,
                        args,
                        stdout,
                        stderr,
                        duration,
                        status
                    ],
                )
                .map_err(|e| e.into())
            })
            .await?;
        }
        Ok(())
    }

    #[instrument]
    pub async fn get_reporter_state(
        &self,
        mon: String,
        rep: String,
    ) -> Result<Option<Vec<u8>>, tokio_rusqlite::Error> {
        type ReporterState = (String, String, Vec<u8>);
        let mut state = Vec::new();

        if let Some(db) = self.db {
            state = db
                .call(move |db| {
                    let mut stmt = db.prepare(
                        "SELECT
                            name,
                            monitor_name,
                            state
                        FROM reporter_state WHERE name == ?1 AND monitor_name = ?2",
                    )?;
                    let state = stmt
                        .query_map([rep, mon], |row| {
                            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                        })?
                        .collect::<std::result::Result<Vec<ReporterState>, rusqlite::Error>>()
                        .unwrap();
                    Ok(state)
                })
                .await?;
        }
        if let Some(s) = state.last() {
            return Ok(Some(s.2.clone()));
        }
        Ok(None)
    }

    #[instrument]
    pub async fn save_reporter_state(
        &self,
        mon: String,
        rep: String,
        state: Vec<u8>,
    ) -> Result<(), tokio_rusqlite::Error> {
        if let Some(db) = self.db {
            db.call(move |db| {
                db.execute(
                    "INSERT INTO
                            reporter_state (
                                name,
                                monitor_name,
                                state
                            )
                            VALUES (
                                ?1,
                                ?2,
                                ?3
                            )
                        ON CONFLICT (name, monitor_name) DO
                        UPDATE
                        SET
                            state = EXCLUDED.state
                        ",
                    params![mon, rep, state],
                )
                .map_err(|e| e.into())
            })
            .await?;
        }
        Ok(())
    }

    /// save the current state of the monitor to db, including:
    ///   - level_index
    ///   - failure_tally
    ///   - success_tally
    ///
    /// updates any previous entry
    #[instrument]
    pub async fn save_monitor_state(
        &self,
        mon: String,
        level_index: usize,
        failure_tally: u64,
        success_tally: u64,
    ) -> Result<(), tokio_rusqlite::Error> {
        if let Some(db) = self.db {
            db.call(move |db| {
                db.execute(
                    "INSERT INTO
                        monitor_state (
                            name,
                            level_index,
                            failure_tally,
                            success_tally
                        )
                        VALUES (
                            ?1,
                            ?2,
                            ?3,
                            ?4
                        )
                    ON CONFLICT (name) DO
                    UPDATE
                    SET
                        level_index = EXCLUDED.level_index,
                        failure_tally = EXCLUDED.failure_tally,
                        success_tally = EXCLUDED.success_tally
                    ",
                    params![mon, level_index, failure_tally, success_tally],
                )
                .map_err(|e| e.into())
            })
            .await?;
        }
        Ok(())
    }

    #[instrument]
    pub async fn get_monitor_state(
        &self,
        mon: String,
    ) -> Result<Option<(usize, u64, u64)>, tokio_rusqlite::Error> {
        type MonitorState = (String, usize, u64, u64);
        let mut state = Vec::new();
        if let Some(db) = self.db {
            state = db
                .call(move |db| {
                    let mut stmt = db.prepare(
                        "SELECT
                        name,
                        level_index,
                        failure_tally,
                        success_tally
                    FROM monitor_state WHERE name == ?1",
                    )?;
                    let state = stmt
                        .query_map([mon], |row| {
                            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                        })?
                        .collect::<std::result::Result<Vec<MonitorState>, rusqlite::Error>>()?;
                    Ok(state)
                })
                .await?;
        }
        if let Some(s) = state.last() {
            return Ok(Some((s.1, s.2, s.3)));
        }
        Ok(None)
    }

    /// prune results table down to the most recent 500
    #[instrument]
    pub async fn prune_results(&self, name: String) -> Result<(), tokio_rusqlite::Error> {
        if let Some(db) = self.db {
            db.call(move |db| {
                db.execute(
                    "DELETE FROM results 
                    WHERE id NOT IN 
                    (
                        SELECT id FROM results 
                        ORDER BY (id) 
                        DESC LIMIT 500
                    )",
                    params![name],
                )
                .map_err(|e| e.into())
            })
            .await?;
        }
        Ok(())
    }
}
