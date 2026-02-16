use crate::monitor::MonitorResult;
use crate::reporters::Reporter;
use async_trait::async_trait;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use tracing::debug;
use tracing::error;
use tracing::instrument;

#[derive(Debug)]
pub struct PostgresqlReporter {
    host: String,
    port: u16,
    user: String,
    password: String,
    db: String,
    conn_count: u32,
    pg_db: PgDb,
}

#[derive(Clone, Deserialize, Debug)]
pub struct PostgresqlReporterArgs {
    host: String,
    port: Option<u16>,
    user: String,
    password: String,
    db: String,
    conn_count: Option<u32>,
}

#[derive(Debug)]
struct PgDb {
    pool: Option<sqlx::Pool<sqlx::Postgres>>,
}

impl PostgresqlReporterArgs {
    pub async fn build(self) -> Result<PostgresqlReporter, sqlx::Error> {
        let mut r = PostgresqlReporter {
            host: self.host,
            port: self.port.unwrap_or(5432),
            user: self.user,
            password: self.password,
            db: self.db,
            conn_count: self.conn_count.unwrap_or(5),
            pg_db: PgDb { pool: None },
        };
        r.initialize_db().await?;
        Ok(r)
    }
}

impl PostgresqlReporter {
    #[instrument]
    async fn initialize_db(&mut self) -> Result<(), sqlx::Error> {
        // Connect to the database.
        let connection_string = format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.host, self.port, self.db
        );
        let pool = PgPoolOptions::new()
            .max_connections(self.conn_count)
            .connect(&connection_string)
            .await?;

        // create synthehol schema if it doesn't exist
        sqlx::query(
            "
                CREATE SCHEMA IF NOT EXISTS synthehol;
            ",
        )
        .execute(&pool)
        .await?;
        debug!("postgresql synthehol schema created/confirmed");

        self.create_results_table().await?;
        debug!("Monitor results table creation succeeded");

        self.pg_db = PgDb { pool: Some(pool) };
        Ok(())
    }

    #[instrument]
    async fn create_monitor_indexes(&self) {}

    #[instrument]
    async fn create_results_table(&self) -> Result<(), sqlx::Error> {
        // create results table if it doesn't exist
        if let Some(p) = &self.pg_db.pool {
            let r = sqlx::query(
                "
                CREATE TABLE IF NOT EXISTS synthehol.monitor_results (
                    id          BIGSERIAL PRIMARY KEY,
                    name        TEXT        NOT NULL,
                    level_name  TEXT        NOT NULL,
                    time        tIMESTAMP   NOT NULL,
                    target      TEXT        NOT NULL,
                    args        TEXT        NOT NULL,
                    stdout      TEXT        NOT NULL,
                    stderr      TEXT        NOT NULL,
                    duration    BIGINT      NOT NULL,
                    status      INTEGER     NOT NULL
                );

            ",
            )
            .execute(p)
            .await?;
            debug!("postgresql monitor table created/confirmed ({r:?})");
        }
        Ok(())
    }
}

#[async_trait]
impl Reporter for PostgresqlReporter {
    #[instrument]
    async fn report(&mut self, output: &MonitorResult) {
        let start_time: i64 = output.start_time.try_into().expect("invalid start time");
        let duration: i64 = output.duration.try_into().expect("invalid duration");

        if let Some(p) = &self.pg_db.pool {
            let r = sqlx::query(
                "
                INSERT INTO synthehol.monitor_results (
                    name,
                    level_name,
                    time,
                    target,
                    args,
                    stdout,
                    stderr,
                    duration,
                    status
                )
                VALUES (
                    $1, $2, to_timestamp($3 / 1000.0), $4, $5, $6, $7, $8, $9
                );
                ",
            )
            .bind(&output.name)
            .bind(&output.level_name)
            .bind(start_time)
            .bind(&output.target)
            .bind(&output.args)
            .bind(&output.stdout)
            .bind(&output.stderr)
            .bind(duration)
            .bind(output.status)
            .execute(p)
            .await;

            match r {
                Ok(r) => {
                    debug!("postgresql report successfully processed ({r:?})");
                }
                Err(e) => {
                    error!("postgresql report failed to process ({e})");
                }
            }
        }
    }

    async fn clear(&mut self, _: &MonitorResult) {
        // nothing to do here
    }

    fn get_state(&self) -> Option<Vec<u8>> {
        // nothing to do here
        None
    }

    fn load_state(&mut self, _: Vec<u8>) {
        // nothing to do here
    }
}
