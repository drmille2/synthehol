use async_trait::async_trait;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::{Command, ExitStatus};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use tracing::{debug, error, info};

/// Represents a monitor that executes a target and reports the result
/// based on the current level
pub struct Monitor {
    pub name: String,
    pub interval: u64,
    levels: Vec<Level>,
    reporters: HashMap<String, Box<dyn Reporter + Send + Sync + 'static>>,
    level_index: usize,
    failure_tally: u64,
    success_tally: u64,
    target: Target,
    running: bool,
    db: Option<tokio_rusqlite::Connection>,
}

#[derive(Deserialize, Debug)]
pub struct MonitorArgs {
    pub name: String,
    pub interval: u64,
    pub level: Vec<LevelArgs>,
    pub target: TargetArgs,
}

impl Monitor {
    pub fn from_args(args: MonitorArgs) -> Self {
        let mut levels = Vec::new();
        for l in args.level.into_iter() {
            levels.push(Level::from_args(l))
        }
        Self {
            name: args.name,
            interval: args.interval,
            levels,
            reporters: HashMap::new(),
            level_index: 0,
            failure_tally: 0,
            success_tally: 0,
            target: Target::from_args(args.target),
            running: false,
            db: None,
        }
    }

    /// Registers a new reporter to the monitor, referenced in levels by name
    pub fn register_reporter(
        &mut self,
        name: &str,
        rep: Box<dyn Reporter + Send + Sync + 'static>,
    ) {
        self.reporters.insert(name.to_string().to_lowercase(), rep);
        info!("[{}] registered reporter: {}", self.name, name);
    }

    pub fn register_db(&mut self, db: tokio_rusqlite::Connection) {
        self.db = Some(db);
        // let path = self.db.as_ref().unwrap().path().unwrap_or("<no db path>");
        info!("[{}] registered database", self.name);
    }

    /// Begin monitoring and reporting loop, shuts down gracefully on cancellation
    pub async fn start(&mut self, cancel: CancellationToken) {
        info!("[{}] starting", self.name);
        self.running = true;
        let sleep = tokio::time::sleep(Duration::from_secs(self.interval));
        tokio::pin!(sleep);
        let mut duration = self.run().await;
        while self.running {
            tokio::select! {
                _ = cancel.cancelled() => { self.stop() }
                _ = &mut sleep => {
                    debug!("[{}] slept {:?}", self.name, duration);
                    let d = Instant::now() + Duration::from_secs(self.interval) - Duration::from_micros(duration);
                    sleep.as_mut().reset(d);
                    duration = self.run().await
                }
            }
        }
    }

    pub fn stop(&mut self) {
        info!("[{}] stopping...", self.name);
        self.running = false;
    }

    // run a single monitor cycle, returning the overall duration
    async fn run(&mut self) -> u64 {
        let start = Instant::now();
        let result = self.execute();
        // if let Ok(res) = result {
        //     self.record_result(res).await;
        // }
        match result {
            Ok(mut r) => {
                if r.status != 0 {
                    self.incr_failure();
                    let l = &self.levels[self.level_index];
                    if l.errors_to_escalate <= self.failure_tally {
                        self.escalate()
                    }
                } else {
                    self.incr_success();
                    let l = &self.levels[self.level_index];
                    if l.successes_to_clear <= self.success_tally {
                        self.clear(&r).await;
                        self.reset()
                    }
                }
                // this needs to be set after we increment the trigger result
                r.level_name = self.levels[self.level_index].name.clone();
                self.report(&r).await;
                match self.record_result(r).await {
                    Ok(_) => {
                        debug!("[{}] recorded result in local db", self.name)
                    }
                    Err(e) => {
                        debug!(
                            "[{}] error while attempting to record result in local db ({})",
                            self.name, e
                        )
                    }
                };
                let stop = Instant::now();
                (stop - start).as_micros() as u64
            }
            Err(e) => {
                error!(e);
                0
            }
        }
    }

    /// a single execution of the monitor target
    fn execute(&self) -> Result<MonitorResult, String> {
        info!("[{}] executing target: {}", self.name, self.target.path);
        let res = self.target.run();
        match res {
            Ok(r) => {
                let args = self.target.args.clone().join(",");
                info!(
                    "[{}] execution completed for target: {} ({} μs)",
                    self.name, self.target.path, r.duration
                );

                // let now = UNIX_EPOCH - Instant::now();
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("System time error")
                    .as_millis() as u64;
                Ok(MonitorResult {
                    name: self.name.clone(),
                    level_name: String::new(),
                    start_time: now, //Instant::now() as i64,
                    stdout: r.stdout,
                    stderr: r.stderr,
                    duration: r.duration,
                    status: r.status.code().unwrap_or(-1),
                    target: self.target.path.clone(),
                    args,
                    // tags: None,
                })
            }
            Err(e) => Err(e),
        }
    }

    /// Increment failure tally and escalate if needed
    fn incr_failure(&mut self) {
        self.success_tally = 0;
        self.failure_tally += 1;
        debug!(
            "[{}] incrementing failure count ({} -> {})",
            self.name,
            self.failure_tally - 1,
            self.failure_tally
        );
    }

    // Increment success tally and clear if needed
    fn incr_success(&mut self) {
        self.failure_tally = 0;
        // this will keep us from incrementing this indefinitely
        // in cases where the monitor never fails
        if self.level_index == 0 {
            self.success_tally = 1;
            return;
        }
        self.success_tally += 1;
        debug!(
            "[{}] incrementing success count ({} -> {})",
            self.name,
            self.success_tally - 1,
            self.success_tally
        );
    }

    /// Escalate to the next level if possible
    fn escalate(&mut self) {
        if self.level_index + 1 < self.levels.len() {
            self.level_index += 1;
        }
        debug!(
            "[{}] escalated monitor level ({} -> {})",
            self.name,
            self.levels[self.level_index - 1].name,
            self.levels[self.level_index].name
        );
    }

    /// Used to reset level & failure tally after a successful monitor run
    fn reset(&mut self) {
        self.level_index = 0;
        self.failure_tally = 0;
        self.success_tally = 0;
        debug!("[{}] reset", self.name);
    }

    async fn record_result(&self, res: MonitorResult) -> Result<(), tokio_rusqlite::Error> {
        // let r = res.clone();
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
        if let Some(db) = self.db.as_ref() {
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

    async fn save(self) -> Result<(), String> {
        if let Some(db) = self.db.as_ref() {
            db.call(move |db| {
                db.execute(
                    "INSERT INTO
                        monitor_state (
                            name,
                            failure_tally,
                            success_tally
                        )
                    VALUES (
                            ?1,
                            ?2,
                            ?3,
                        )
                    ON CONFLICT (email) DO
                    UPDATE
                    SET
                        name = excluded.name,
                        failure_tally = excluded.failure_tally,
                        success_tally = excluded.success_tally
                    ",
                    params![self.name, self.failure_tally, self.success_tally],
                )
                .map_err(|e| e.into())
            })
            .await
            .map_err(|e| format!("failed to save monitor state ({})", e))?;
        }
        Ok(())
    }

    async fn load(self) -> Result<(), String> {
        if let Some(db) = self.db.as_ref() {
            // db.call(move |db| {
            //     let mut stmt = db.prepare(
            //         "SELECT
            //             name,
            //             failure_tally,
            //             success_tally
            //         FROM monitor_state",
            //     )?;
            //     let person_iter = stmt.query_map([], |row| {
            //         Ok(Person {
            //             id: row.get(0)?,
            //             name: row.get(1)?,
            //             data: row.get(2)?,
            //         })
            //     })?;
            // })
            // .await
            // .map_err(|e| format!("failed to load monitor state ({})", e))?;
        }
        Ok(())
    }

    /// Dispatch all reporters based on current level
    async fn report(&self, res: &MonitorResult) {
        let l = &self.levels[self.level_index];
        for k in l.reporters.iter() {
            let k_l = k.clone().to_lowercase();
            if let Some(r) = &self.reporters.get(&k_l) {
                r.report(res).await;
            }
        }
    }

    /// Clear all reporters based on current level
    async fn clear(&self, res: &MonitorResult) {
        let l = &self.levels[self.level_index];
        for k in l.reporters.iter() {
            let k_l = k.clone().to_lowercase();
            if let Some(r) = &self.reporters.get(&k_l) {
                r.clear(res).await;
            }
        }
    }
}

/// Levels contain a list of reporter names that will be attempted
/// after the parent monitor target executes, and optionally a number
/// of allowed failures before escalation should be tried
#[derive(Debug)]
struct Level {
    name: String,
    errors_to_escalate: u64,
    successes_to_clear: u64,
    reporters: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct LevelArgs {
    name: String,
    errors_to_escalate: Option<u64>,
    successes_to_clear: Option<u64>,
    reporters: Vec<String>,
}

impl Level {
    fn from_args(args: LevelArgs) -> Self {
        Self {
            name: args.name,
            errors_to_escalate: args.errors_to_escalate.unwrap_or(1),
            successes_to_clear: args.successes_to_clear.unwrap_or(1),
            reporters: args.reporters,
        }
    }
}

/// A monitor execution target, such as a script or binary that produces
/// some output and a 0 result code indicating success, and >0 for failure.
/// Command-line arguments are provided by a vec of strings, and environment
/// variables by a vec of (String, String) tuples
#[derive(Debug)]
struct Target {
    path: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
}

#[derive(Deserialize, Debug)]
pub struct TargetArgs {
    pub path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug)]
struct TargetOutput {
    stdout: String,
    stderr: String,
    duration: u64,
    status: ExitStatus,
}

impl Target {
    fn from_args(args: TargetArgs) -> Self {
        Self {
            path: args.path,
            args: args.args,
            env: args.env,
        }
    }

    /// Run the target, returning duration and other execution details
    #[instrument(level=tracing::Level::DEBUG)]
    fn run(&self) -> Result<TargetOutput, String> {
        let env = self.env.clone();
        let start = Instant::now();
        let mut cmd = Command::new(&self.path);
        let output = cmd
            .args(&self.args)
            .envs(env)
            .output()
            .map_err(|e| format!("failed to run target ({0})", e))?;
        let stop = Instant::now();
        let duration = (stop - start).as_micros() as u64;
        let status = output.status;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let out = TargetOutput {
            stdout,
            stderr,
            duration,
            status,
        };
        // dbg!(&out);
        Ok(out)
    }
}

pub type ReporterArgs = toml::Table;

/// Reporters have an async report() function that handles a monitor result
/// taking care of any formatting and delivery required
#[async_trait]
pub trait Reporter {
    async fn report(&self, _: &MonitorResult);
    async fn clear(&self, _: &MonitorResult);
    fn state(&self) -> Option<Vec<u8>>;
    fn restore(&mut self, _: Option<Vec<u8>>) -> Result<(), String>;
}

#[derive(Debug, Serialize, Clone)]
pub struct MonitorResult {
    pub name: String,
    pub level_name: String,
    pub start_time: u64,
    pub target: String,
    pub args: String,
    pub stdout: String,
    pub stderr: String,
    pub duration: u64,
    pub status: i32,
    // pub tags: Option<Vec<(String, String)>>,
}
