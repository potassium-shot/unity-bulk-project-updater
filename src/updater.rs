use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::LazyLock,
};

use anyhow::Result;
use regex::Regex;

#[derive(Debug)]
pub struct Updater {
    pub queue: Vec<Update>,
    unity: PathBuf,
    max_process: usize,
}

impl Updater {
    const UNITY_VERSION_REGEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^\d{4}\.\d+\.\d+f\d+$").unwrap());

    pub fn find_avaible_unity_versions(unity_path: &Path) -> Result<Vec<String>> {
        Ok(std::fs::read_dir(unity_path)?
            .filter_map(|path| {
                if let Ok(path) = path {
                    if let Ok(ty) = path.file_type() {
                        let name = path.file_name().to_string_lossy().to_string();

                        if ty.is_dir() && Self::UNITY_VERSION_REGEX.is_match(&name) {
                            Some(name)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect())
    }

    pub fn new(version_path: &Path, max_process: usize) -> Self {
        Self {
            queue: Vec::new(),
            unity: version_path.join("Editor/Unity.exe"),
            max_process,
        }
    }

    pub fn add_to_queue(&mut self, project_path: PathBuf) {
        self.queue.push(Update {
            project: project_path,
            state: UpdateState::Pending,
        });
    }

    pub fn update(&mut self) -> bool {
        // Check the queue for finished processing updates

        let mut amount_processing = 0;

        for update in self
            .queue
            .iter_mut()
            .filter(|update| update.state.kind() == UpdateStateKind::Processing)
        {
            match &mut update.state {
                UpdateState::Processing(child) => {
                    amount_processing += 1;
                    match child.try_wait() {
                        Ok(Some(exit_code)) if exit_code.code() == Some(0) => {
                            update.state = UpdateState::Success;
                        }
                        Ok(Some(exit_code)) => {
                            update.state = UpdateState::Error(format!(
                                "Unity exited with code {}.",
                                exit_code
                            ));
                        }
                        Ok(None) => {}
                        Err(error) => {
                            update.state = UpdateState::Error(format!(
                                "Could not fetch unity state: {}",
                                error
                            ))
                        }
                    }
                }
                _ => unreachable!(),
            }
        }

        // Start pending updates

        for update in self
            .queue
            .iter_mut()
            .filter(|update| update.state.kind() == UpdateStateKind::Pending)
            .take(self.max_process.saturating_sub(amount_processing))
        {
            let process = Command::new(&self.unity)
                .arg("-batchmode")
                .arg("-nographics")
                .arg("-quit")
                .arg("-projectPath")
                .arg(&update.project)
                .spawn();

            update.state = match process {
                Ok(child) => UpdateState::Processing(child),
                Err(error) => {
                    UpdateState::Error(format!("Could not start the unity process: {}", error))
                }
            }
        }

        amount_processing > 0
    }
}

#[derive(Debug)]
pub struct Update {
    pub project: PathBuf,
    pub state: UpdateState,
}

#[derive(Debug, kinded::Kinded)]
pub enum UpdateState {
    Pending,
    Processing(Child),
    Success,
    Error(String),
}
