use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::{Command, ExitStatus};
use std::time::Instant;

pub struct Monitor {
    pub name: String,
    pub interval: u64,
    levels: Vec<Level>,
    reporters: HashMap<String, Box<dyn Reporter>>,
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
            reporters: HashMap::new(), // add reporters here
            level_index: 0,
            failure_tally: 0,
            target: Target::from_args(args.target),
        }
    }

    pub fn register_reporter(&mut self, name: String, rep: Box<dyn Reporter>) {
        self.reporters.insert(name, rep);
    }

    pub fn start(&mut self) -> Result<(), ()> {
        Ok(())
    }

    fn run(&self) -> MonitorResult {
        let TargetOutput {
            stdout,
            stderr,
            duration,
            status,
        } = self.target.run();
        MonitorResult {
            name: self.name.clone(),
            stdout,
            stderr,
            duration,
            status,
            tags: None,
        }
    }

    fn incr_failure(&mut self) {
        self.failure_tally += 1;
        let l = &self.levels[self.level_index as usize];
        if let Some(esc) = l.errors_to_escalate {
            if esc <= self.failure_tally {
                self.escalate()
            }
        }
    }

    fn escalate(&mut self) {
        if self.level_index + 1 < self.levels.len() as u64 {
            self.level_index += 1;
        }
    }

    fn reset(&mut self) {
        self.level_index = 0;
        self.failure_tally = 0;
    }

    fn report(&self, res: &MonitorResult) {
        let l = &self.levels[self.level_index as usize];
        for k in l.reporters.iter() {
            if let Some(r) = &self.reporters.get(k) {
                let _ = r.report(res); // Probably needs to be async and awaited
            }
        }
    }
}

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

    fn run(&self) -> TargetOutput {
        let env = self.env.clone();
        let start = Instant::now();
        let mut cmd = Command::new(&self.path);
        let output = cmd.args(&self.args).envs(env).output().unwrap(); // TODO: handle it
        let stop = Instant::now();
        let duration = (stop - start).as_secs();
        let status = output.status;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        TargetOutput {
            stdout,
            stderr,
            duration,
            status,
        }
    }
}

pub type ReporterArgs = toml::Table;

#[async_trait]
pub trait Reporter {
    async fn report(&self, _: &MonitorResult);
    fn format(&self, _: &MonitorResult) -> String;
}

pub struct MonitorResult {
    pub name: String,
    pub stdout: String,
    pub stderr: String,
    pub duration: u64,
    pub status: ExitStatus,
    pub tags: Option<Vec<(String, String)>>,
}

impl MonitorResult {
    pub fn new(
        name: String,
        stdout: String,
        stderr: String,
        duration: u64,
        status: ExitStatus,
        tags: Option<Vec<(String, String)>>,
    ) -> Self {
        Self {
            name,
            stdout,
            stderr,
            duration,
            status,
            tags,
        }
    }
}
