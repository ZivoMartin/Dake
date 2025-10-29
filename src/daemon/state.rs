use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    sync::Arc,
    time::Duration,
};

use anyhow::{Result, bail};
use derive_getters::Getters;
use notifier_hub::notifier::NotifierHub;
use tokio::{select, sync::Mutex, time::sleep};
use tracing::{info, warn};

use crate::{
    daemon::{Notif, process_datas::ProcessDatas},
    network::SocketAddr,
    process_id::{ProcessId, ProjectId},
};

type Wrapped<T> = Arc<Mutex<T>>;

#[derive(Clone, Getters)]
pub struct State {
    id_database: Wrapped<HashMap<ProjectId, u64>>,
    notifier_hub: Wrapped<NotifierHub<Arc<Notif>, ProcessId>>,
    processes: Wrapped<HashMap<ProcessId, ProcessDatas>>,
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
    pub fn new(daemon_sock: SocketAddr) -> Self {
        Self {
            daemon_sock,
            id_database: Wrapped::default(),
            notifier_hub: Wrapped::default(),
            processes: Wrapped::default(),
        }
    }

    pub async fn remove_process(&self, pid: &ProcessId) -> Result<Option<ProcessDatas>> {
        let processes = self.processes.clone();
        let sleep_fut = Box::pin(sleep(Duration::from_secs(5)));
        select! {
            _ = sleep_fut => {
                bail!("Failed to lock processes database.");
            }
            mut processes = processes.lock() => {
                Ok(processes.remove(pid))
            }
        }
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
        let sleep_fut = Box::pin(sleep(Duration::from_secs(5)));
        select! {
            _ = sleep_fut => {
                warn!("Failed to lock processes database")
            }
            mut processes = processes.lock() => {
                processes.insert(pid.clone(), datas.clone());
                info!("{datas:?} has been registered for the pid {pid:?}.");
            }
        }
    }

    pub async fn read_process_data(&self, pid: &ProcessId) -> Result<Option<ProcessDatas>> {
        info!("Trying to fetch the process datas for {pid:?}.");
        let sleep_fut = Box::pin(sleep(Duration::from_secs(5)));
        select! {
            _ = sleep_fut => {
                bail!("Time out when unlocking the processes database.")
            }
            processes = self.processes.lock() => {
                info!("Successfully locked the processes database for {pid:?}.");
                Ok(processes.get(pid).cloned())
            }
        }
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
        let sleep_fut = Box::pin(sleep(Duration::from_secs(5)));
        select! {
            _ = sleep_fut => {
                bail!("Time out when unlocking the clients database.")
            }
            mut id_database = self.id_database.lock() => {
                let entry = id_database.entry(project_id).and_modify(|e| *e += 1).or_insert(2);
                let id = *entry - 1;
                Ok(id)
            }
        }
    }
}
