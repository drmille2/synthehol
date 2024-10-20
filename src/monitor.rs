use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tracing::event;
use tracing::Level as tLevel;

/// Represents a monitor that executes a target and reports the result
/// based on the current level
pub struct Monitor {
    pub name: String,
    pub interval: u64,
    levels: Vec<Level>,
    // reporters: HashMap<String, Box<dyn Reporter>>,
    reporters: HashMap<String, Box<dyn Reporter + Send + Sync + 'static>>,
    level_index: u64,
    failure_tally: u64,
    target: Target,
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
            target: Target::from_args(args.target),
        }
    }

    /// Registers a new reporter to the monitor, referenced in levels by name
    pub fn register_reporter(
        &mut self,
        name: &str,
        rep: Box<dyn Reporter + Send + Sync + 'static>,
    ) {
        self.reporters.insert(name.to_string().to_lowercase(), rep);
        let msg = format!("registered reporter {0}", name);
        event!(tLevel::INFO, msg);
    }

    /// Begin monitoring and reporting loop, does not terminate
    /// TODO: add a .stop() method
    pub async fn start(&mut self) {
        let msg = format!("starting monitor {0}", self.name);
        event!(tLevel::INFO, msg);
        let mut sleep = Duration::new(0, 0);
        loop {
            let res = self.run();
            match res {
                Ok(r) => {
                    if !r.status.success() {
                        self.incr_failure();
                    } else {
                        self.reset();
                    }
                    self.report(&r).await;
                    sleep = Duration::from_secs(self.interval) - Duration::from_micros(r.duration);
                }
                Err(e) => {
                    event!(tLevel::WARN, e);
                }
            }

            thread::sleep(sleep);
        }
    }

    /// Run a single execution of the monitor target
    fn run(&self) -> Result<MonitorResult, String> {
        let res = self.target.run();
        match res {
            Ok(r) => Ok(MonitorResult {
                name: self.name.clone(),
                level_name: self.levels[self.level_index as usize].name.clone(),
                stdout: r.stdout,
                stderr: r.stderr,
                duration: r.duration,
                status: r.status,
                // tags: None,
            }),
            Err(e) => Err(e),
        }
    }

    /// Increment failure tally and escalate if needed
    fn incr_failure(&mut self) {
        self.failure_tally += 1;
        let msg = format!(
            "incrementing failure count (was {0}, now {1})",
            self.failure_tally - 1,
            self.failure_tally
        );
        event!(tLevel::INFO, msg);
        let l = &self.levels[self.level_index as usize];
        if let Some(esc) = l.errors_to_escalate {
            if esc <= self.failure_tally {
                self.escalate()
            }
        }
    }

    /// Escalate to the next level if possible
    fn escalate(&mut self) {
        if self.level_index + 1 < self.levels.len() as u64 {
            self.level_index += 1;
        }
        let msg = format!(
            "escalated monitor level (was {0}, now {1})",
            self.levels[self.level_index as usize - 1].name,
            self.levels[self.level_index as usize].name,
        );
        event!(tLevel::INFO, msg);
    }

    /// Used to reset level & failure tally after a successful monitor run
    fn reset(&mut self) {
        self.level_index = 0;
        self.failure_tally = 0;
        event!(tLevel::INFO, "reset monitor level & failure count");
    }

    /// Dispatch all reporters based on current level
    async fn report(&self, res: &MonitorResult) {
        let l = &self.levels[self.level_index as usize];
        for k in l.reporters.iter() {
            let k_l = k.clone().to_lowercase();
            if let Some(r) = &self.reporters.get(&k_l) {
                r.report(res).await;
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
    errors_to_escalate: Option<u64>,
    reporters: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct LevelArgs {
    name: String,
    errors_to_escalate: Option<u64>,
    reporters: Vec<String>,
}

impl Level {
    fn from_args(args: LevelArgs) -> Self {
        Self {
            name: args.name,
            errors_to_escalate: args.errors_to_escalate,
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
    fn run(&self) -> Result<TargetOutput, String> {
        let env = self.env.clone();
        let start = Instant::now();
        let mut cmd = Command::new(&self.path);
        let output = cmd
            .args(&self.args)
            .envs(env)
            .output()
            .map_err(|e| format!("failed to run target ({0})", e))?; // TODO: handle it
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
        let msg = format!("invoked monitor target \"{0}\"", self.path);
        event!(tLevel::INFO, msg);
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
}

#[derive(Debug)]
pub struct MonitorResult {
    pub name: String,
    pub level_name: String,
    pub stdout: String,
    pub stderr: String,
    pub duration: u64,
    pub status: ExitStatus,
    // pub tags: Option<Vec<(String, String)>>,
}
