use crate::outputs;
use serde::Deserialize;
use std::process::{Command, ExitStatus};

#[derive(Deserialize)]
pub struct MonitorArgs {
    pub name: String,
    pub levels: Vec<LevelArgs>,
    pub target: TargetArgs,
    pub interval: u32,
}

#[derive(Deserialize)]
pub struct LevelArgs {
    pub name: String,
    pub errors_to_escalate: Option<u32>,
    pub outputs: Vec<String>,
}

pub struct Monitor {
    pub name: String,
    pub levels: Vec<Level>,
    pub interval: u32,
    target: Target,
}

#[derive(Deserialize)]
pub struct TargetArgs {
    pub path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl Monitor {
    pub fn new(name: String, levels: Vec<Level>, interval: u32, target: Target) -> Self {
        Self {
            name,
            levels,
            interval,
            target,
        }
    }
}

fn parse_level(args: LevelArgs) -> Level {
    Level::new(args.name, args.errors_to_escalate, Vec::new())
}

struct Level {
    name: String,
    errors_to_escalate: Option<u32>,
    outputs: Vec<Box<dyn Reporter>>,
}

impl Level {
    fn new(name: String, errors_to_escalate: Option<u32>, outputs: Vec<Box<dyn Reporter>>) -> Self {
        Self {
            name,
            errors_to_escalate,
            outputs,
        }
    }
}

#[derive(Deserialize)]
pub struct ReporterArgs {
    name: String,
    config: toml::Table,
}

pub trait Reporter {
    // fn report(&self, _: MonResult);
    fn format(&self, _: MonResult) -> String;
}

pub struct MonResult {
    stdout: String,
    stderr: String,
    duration: u32,
    result: ExitStatus,
    target: Target,
    tags: Option<Vec<(String, String)>>,
}

impl MonResult {
    fn new(
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

pub struct Target {
    pub path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl Target {
    fn new(path: String, args: Vec<String>, env: Vec<(String, String)>) -> Self {
        Self { path, args, env }
    }

    fn run(self) -> MonResult {
        let env = self.env.clone();
        let mut cmd = Command::new(&self.path);
        let output = cmd.args(&self.args).envs(env).output().unwrap(); // TODO: handle it
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        MonResult::new(stdout, stderr, 0, output.status, self, None)
    }
}
