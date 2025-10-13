use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Result, bail};
use tokio::{select, spawn, sync::Mutex, time::sleep};
use tracing::{info, warn};

use crate::{network::process_datas::ProcessDatas, process_id::ProcessId};

#[derive(Clone, Default)]
pub struct State {
    processes: Arc<Mutex<HashMap<ProcessId, ProcessDatas>>>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_process(&self, pid: ProcessId, datas: ProcessDatas) {
        info!("Registering new process {datas:?} the pid {pid:?}.");
        let processes = self.processes.clone();
        spawn(async move {
            let sleep_fut = Box::pin(sleep(Duration::from_secs(5)));
            select! {
                _ = sleep_fut => {
                    warn!("Failed to lock client database, {datas:?} will not be registered for {pid:?}")
                }
                mut processes = processes.lock() => {
                    processes.insert(pid.clone(), datas.clone());
                    info!("{datas:?} has been registered for the pid {pid:?}.");
                }
            }
        });
    }

    pub async fn read_process_data(&self, pid: &ProcessId) -> Result<Option<ProcessDatas>> {
        info!("Trying to fetch the process datas for {pid:?}.");
        let sleep_fut = Box::pin(sleep(Duration::from_secs(5)));
        select! {
            _ = sleep_fut => {
                bail!("Time out when unlocking the clients database.")
            }
            processes = self.processes.lock() => {
                info!("Successfully locked the clients database for {pid:?}.");
                Ok(processes.get(pid).cloned())
            }
        }
    }

    pub async fn read_client(&self, pid: &ProcessId) -> Result<Option<SocketAddr>> {
        Ok(if let Some(datas) = self.read_process_data(pid).await? {
            datas.client_sock().clone()
        } else {
            None
        })
    }

    pub async fn read_involved_hosts(&self, pid: &ProcessId) -> Result<Option<Vec<SocketAddr>>> {
        Ok(if let Some(datas) = self.read_process_data(pid).await? {
            Some(datas.involved_hosts)
        } else {
            None
        })
    }

    pub async fn read_client_or_warn(&self, pid: &ProcessId) -> Option<SocketAddr> {
        info!("Reading client (or warn) for {pid:?}");
        match self.read_process_data(&pid).await {
            Ok(Some(datas)) => {
                if datas.is_remote() {
                    warn!("Tried to read the client socket from a remote host.");
                }
                datas.client_sock().clone()
            }
            Ok(None) => {
                warn!(
                    "Failed to fetch the client socket for the process {pid:?}, the process is not registered."
                );
                None
            }
            Err(_) => {
                warn!(
                    "Failed to fetch the client socket for the process {pid:?}, unable to lock the database."
                );
                None
            }
        }
    }
}
