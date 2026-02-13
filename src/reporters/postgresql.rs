use crate::monitor::MonitorResult;
use crate::reporters::{util, Reporter};
use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use tracing::debug;
use tracing::error;
use tracing::instrument;

#[derive(Debug)]
pub struct PostgresqlReporter {
    host: String,
    port: i16,
    user: String,
    password: String,
    db: String,
    conn_count: u32,
    pg_db: PgDb,
}

#[derive(Debug)]
struct PgDb {
    pool: Option<sqlx::Pool<sqlx::Postgres>>,
}

impl PostgresqlReporter {
    pub async fn from_toml(config: &toml::Table) -> Result<Self, util::ConfigError> {
        let host = util::get_str_or_else(config, "host", None)?;
        let port = util::get_int_or_else(config, "port", Some(5432))?.try_into()?;
        let user = util::get_str_or_else(config, "user", None)?;
        let password = util::get_str_or_else(config, "password", None)?;
        let db = util::get_str_or_else(config, "db", None)?;
        let conn_count = util::get_int_or_else(config, "conn_count", Some(5))?.try_into()?;

        let mut r = Self {
            host,
            port,
            user,
            password,
            db,
            conn_count,
            pg_db: PgDb { pool: None },
        };
        _ = r.initialize_db().await;
        Ok(r)
    }

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
        // TODO
    }

    fn get_state(&self) -> Option<Vec<u8>> {
        // TODO
        None
    }

    fn load_state(&mut self, _: Vec<u8>) {
        // TODO
    }
}
