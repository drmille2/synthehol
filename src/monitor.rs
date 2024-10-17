// use async_trait::async_trait;
use serde::Deserialize;
use std::process::{Command, ExitStatus};

pub struct Monitor<T>
where
    T: Reporter,
{
    pub name: String,
    levels: Vec<Level<T>>,
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

impl<T> Monitor<T>
where
    T: Reporter,
{
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
}

struct Level<T>
where
    T: Reporter,
{
    name: String,
    errors_to_escalate: Option<u32>,
    reporters: Vec<Box<T>>,
}

#[derive(Deserialize, Debug)]
pub struct LevelArgs {
    name: String,
    errors_to_escalate: Option<u32>,
    reporters: Vec<String>,
}

impl<T> Level<T>
where
    T: Reporter,
{
    fn from_args(args: LevelArgs) -> Self {
        Self {
            name: args.name,
            errors_to_escalate: args.errors_to_escalate,
            reporters: Vec::new(), // TODO: do the reporters
        }
    }
}

struct Target {
    pub path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Deserialize, Debug)]
pub struct TargetArgs {
    pub path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl Target {
    fn from_args(args: TargetArgs) -> Self {
        Self {
            path: args.path,
            args: args.args,
            env: args.env,
        }
    }

    fn run(self) -> MonitorResult {
        let env = self.env.clone();
        let mut cmd = Command::new(&self.path);
        let output = cmd.args(&self.args).envs(env).output().unwrap(); // TODO: handle it
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        MonitorResult::new(stdout, stderr, 0, output.status, self, None)
    }
}

pub type ReporterArgs = toml::Table;

// #[async_trait]
pub trait Reporter {
    async fn report(&self, _: MonitorResult);
    fn format(&self, _: MonitorResult) -> String;
}

pub struct MonitorResult {
    pub stdout: String,
    pub stderr: String,
    pub duration: u32,
    pub result: ExitStatus,
    pub target: Target,
    pub tags: Option<Vec<(String, String)>>,
}

impl MonitorResult {
    pub fn new(
        stdout: String,
        stderr: String,
        duration: u32,
        result: ExitStatus,
        target: Target,
        tags: Option<Vec<(String, String)>>,
    ) -> Self {
        Self {
            stdout,
            stderr,
            duration,
            result,
            target,
            tags,
        }
    }
}
