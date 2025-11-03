use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Formatter},
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use notifier_hub::notifier::NotifierHub;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{
    constants::{CHANNEL_SIZE, INITIAL_PROCESS_ID},
    daemon::{Notif, memory::config::DaemonConfig, process_datas::ProcessDatas},
    lock,
    network::SocketAddr,
    process_id::{ProcessId, ProjectId},
};

type Wrapped<T> = Arc<Mutex<T>>;
type Hub = Wrapped<NotifierHub<Arc<Notif>, ProcessId>>;
type IdDatabase = Wrapped<HashMap<ProjectId, u64>>;
type ProcessesDatabase = Wrapped<HashMap<ProcessId, ProcessDatas>>;
type TargetLocksSet = Wrapped<HashSet<(ProjectId, String)>>;

#[derive(Clone)]
pub struct State {
    id_database: IdDatabase,
    target_locks: TargetLocksSet,
    notifier_hub: Hub,
    processes: ProcessesDatabase,
    config: DaemonConfig,
    pub daemon_sock: SocketAddr,
}

impl Debug for State {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("daemon_sock", &self.daemon_sock)
            .finish()
    }
}

impl State {
    pub fn new(daemon_sock: SocketAddr) -> Result<Self> {
        if DaemonConfig::is_running() {
            bail!("Daemon is already running.")
        }

        let config = DaemonConfig::load_or_generate().context("Failed to generate config.")?;

        Ok(Self {
            daemon_sock,
            config: config,
            target_locks: Wrapped::default(),
            id_database: Wrapped::default(),
            notifier_hub: Wrapped::default(),
            processes: Wrapped::default(),
        })
    }

    pub fn config(&mut self) -> Result<DaemonConfig> {
        Ok(self.config)
    }

    pub fn notifier_hub(&self) -> &Hub {
        &self.notifier_hub
    }

    pub fn id_database(&self) -> &IdDatabase {
        &self.id_database
    }

    pub fn processes(&self) -> &ProcessesDatabase {
        &self.processes
    }

    pub fn daemon_sock(&self) -> &SocketAddr {
        &self.daemon_sock
    }

    pub async fn remove_process(&self, pid: &ProcessId) -> Result<Option<ProcessDatas>> {
        let processes_ref = self.processes.clone();
        let mut processes = lock!(processes_ref).await?;
        Ok(processes.remove(pid))
    }

    // Register the process in the database with a default ProcessData value
    pub async fn register_process(&self, pid: ProcessId) {
        info!("Registering new process {pid:?}.");
        self.set_process_datas(pid.clone(), ProcessDatas::default())
            .await;
        info!("{pid:?} has been registered.");
    }

    pub async fn set_process_datas(&self, pid: ProcessId, datas: ProcessDatas) {
        info!("Setting process datas {datas:?} for process {pid:?}.");
        let processes = self.processes.clone();
        match lock!(processes).await {
            Ok(mut processes) => {
                processes.insert(pid.clone(), datas.clone());
                info!("{datas:?} has been registered for the pid {pid:?}.");
            }
            Err(_) => warn!("Failed to lock processes database"),
        }
    }

    pub async fn read_process_data(&self, pid: &ProcessId) -> Result<Option<ProcessDatas>> {
        info!("Trying to fetch the process datas for {pid:?}.");
        let processes = self.processes.clone();
        let processes = lock!(processes).await?;
        info!("Successfully locked the processes database for {pid:?}.");
        Ok(processes.get(pid).cloned())
    }

    pub async fn process_is_registered(&self, pid: &ProcessId) -> Result<bool> {
        info!("Trying to learn if {pid:?} is registered.");
        Ok(self.read_process_data(pid).await?.is_some())
    }

    pub async fn read_involved_hosts(&self, pid: &ProcessId) -> Result<Option<Vec<SocketAddr>>> {
        Ok(if let Some(datas) = self.read_process_data(pid).await? {
            Some(datas.involved_hosts)
        } else {
            None
        })
    }

    pub async fn read_args(&self, pid: &ProcessId) -> Result<Option<Vec<String>>> {
        Ok(if let Some(datas) = self.read_process_data(pid).await? {
            Some(datas.args)
        } else {
            None
        })
    }

    pub async fn get_fresh_id(&self, project_id: ProjectId) -> Result<u64> {
        let id_database = self.id_database.clone();
        let mut id_database = lock!(id_database).await?;
        let entry = id_database
            .entry(project_id)
            .and_modify(|e| *e += 1)
            .or_insert(INITIAL_PROCESS_ID + 1);
        let id = *entry - 1;
        Ok(id)
    }

    #[tracing::instrument(skip(self), fields(%project_id, %target))]
    pub async fn unlock_target(&self, project_id: ProjectId, target: String) -> Result<()> {
        info!("Attempting to unlock target...");

        {
            let locks = self.target_locks.clone();
            let mut locks = lock!(locks).await?;
            info!("Acquired lock on target_locks");

            if !locks.remove(&(project_id.clone(), target.clone())) {
                warn!("Target was already unlocked: {target}");
            } else {
                info!("Successfully removed lock for target: {target}");
            }
        }

        let hub = self.notifier_hub.clone();
        let hub = lock!(hub).await?;
        info!("Acquired lock on notifier_hub");

        info!("Broadcasting unlock notification for target: {target}");

        hub.arc_send(
            Notif::TargetUnlock {
                target: target.clone(),
            },
            &ProcessId {
                id: 0,
                project_id: project_id.clone(),
            },
        )
        .context("Failed to broadcast the unlock notification")?;

        info!("Unlock notification sent successfully for project {project_id}");

        Ok(())
    }

    // This function does not take any duration because the only time we need to wait for something
    // is when a build is running. However, this build might take up to 13 hours if the user wishes,
    // so it is practically impossible to set a timeout for this lock.
    #[tracing::instrument(skip(self), fields(%target))]
    pub async fn lock_target(&self, project_id: ProjectId, target: String) -> Result<()> {
        loop {
            info!("Attempting to acquire lock for target");

            let free: bool = {
                let locks = self.target_locks.clone();
                let mut locks = lock!(locks).await?;
                let was_free = locks.insert((project_id.clone(), target.clone()));

                info!(?was_free, "Lock table updated");
                was_free
            };

            if free {
                info!("Lock acquired successfully for target");
                break Ok(());
            } else {
                info!("Target already locked, waiting for unlock notification");

                let mut subscriber = {
                    let hub = self.notifier_hub.clone();
                    let mut hub = lock!(hub).await?;

                    // Little trick here: process 0 is used as a broadcast channel for the project.
                    let sub = hub.subscribe(
                        &ProcessId {
                            id: 0,
                            project_id: project_id.clone(),
                        },
                        CHANNEL_SIZE,
                    );

                    info!("Subscribed to project broadcast channel");
                    sub
                };

                loop {
                    info!("Waiting for TargetUnlock notification...");
                    match subscriber
                        .recv()
                        .await
                        .context("Failed to wait for target unlocking")
                    {
                        Ok(notif) => {
                            notif.trace();
                            if matches!(
                                notif.as_ref(),
                                Notif::TargetUnlock { target: target_unlocked }
                                if &target == target_unlocked
                            ) {
                                info!("Target unlocked, retrying lock acquisition");
                                break;
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "Error while waiting for target unlock");
                            break;
                        }
                    }
                }
            }
        }
    }
}
