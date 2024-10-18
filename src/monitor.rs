use async_trait::async_trait;
use serde::Deserialize;
use std::process::{Command, ExitStatus};
use std::time::Instant;

pub struct Monitor {
    pub name: String,
    levels: Vec<Level>,
    pub interval: u32,
    target: Target,
}

#[derive(Deserialize, Debug)]
pub struct MonitorArgs {
    pub name: String,
    pub interval: u32,
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
            levels,
            target: Target::from_args(args.target),
            interval: args.interval,
        }
    }

    pub fn run(&self) -> MonitorResult {
        let output = self.target.run();
        MonitorResult {
            name: self.name.clone(),
            stdout: output.stdout,
            stderr: output.stderr,
            duration: output.duration,
            status: output.status,
            tags: None,
        }
    }
}

struct Level {
    name: String,
    errors_to_escalate: Option<u32>,
    reporters: Vec<Box<dyn Reporter>>,
}

#[derive(Deserialize, Debug)]
pub struct LevelArgs {
    name: String,
    errors_to_escalate: Option<u32>,
    reporters: Vec<String>,
}

impl Level {
    fn from_args(args: LevelArgs) -> Self {
        Self {
            name: args.name,
            errors_to_escalate: args.errors_to_escalate,
            reporters: Vec::new(), // TODO: do the reporters
        }
    }

    fn report(&self, res: MonitorResult) {
        for n in self.reporters.iter() {
            n.report(&res);
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
