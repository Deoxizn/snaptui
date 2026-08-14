use std::collections::HashMap;

use anyhow::{anyhow, Result};
use zbus::blocking::{Connection, Proxy};

pub const SERVICE: &str = "org.opensuse.Snapper";
pub const PATH: &str = "/org/opensuse/Snapper";
pub const INTERFACE: &str = "org.opensuse.Snapper";

#[derive(Debug, Clone, PartialEq)]
pub struct SnapperConfig {
    pub name: String,
    pub subvolume: String,
    pub values: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub id: u32,
    pub kind: u8,
    pub date: i64,
    pub uid: u32,
    pub description: String,
    pub cleanup: String,
}

pub struct SnapperDb {
    proxy: Proxy<'static>,
}

impl SnapperDb {
    pub fn connect() -> Result<Self> {
        let conn = Connection::system().map_err(|e| anyhow!("dbus system bus: {e}"))?;
        let proxy = Proxy::new(&conn, SERVICE, PATH, INTERFACE)
            .map_err(|e| anyhow!("dbus proxy: {e}"))?;
        Ok(Self { proxy })
    }

    pub fn list_configs(&self) -> Result<Vec<SnapperConfig>> {
        let reply = self.proxy.call_method("ListConfigs", &())?;
        let entries: Vec<(String, String, HashMap<String, String>)> =
            reply.body().deserialize()?;
        Ok(entries
            .into_iter()
            .map(|(name, subvolume, values)| SnapperConfig {
                name,
                subvolume,
                values,
            })
            .collect())
    }

    pub fn list_snapshots(&self, config: &str) -> Result<Vec<Snapshot>> {
        let reply = self.proxy.call_method("ListSnapshots", &config)?;
        let entries: Vec<(u32, u16, u32, i64, u32, String, String, HashMap<String, String>)> =
            reply.body().deserialize()?;
        let mut snaps: Vec<Snapshot> = entries
            .into_iter()
            .map(
                |(id, kind, _pre, date, uid, description, cleanup, _userdata)| Snapshot {
                    id,
                    kind: kind as u8,
                    date,
                    uid,
                    description,
                    cleanup,
                },
            )
            .collect();
        snaps.sort_by_key(|s| s.id);
        Ok(snaps)
    }

    pub fn get_used_space(&self, config: &str, id: u32) -> Result<u64> {
        let reply = self.proxy.call_method("GetUsedSpace", &(config, id))?;
        let bytes: u64 = reply.body().deserialize()?;
        Ok(bytes)
    }

    pub fn create_single(
        &self,
        config: &str,
        description: &str,
        cleanup: &str,
    ) -> Result<u32> {
        let userdata: HashMap<String, String> = HashMap::new();
        let reply = self
            .proxy
            .call_method("CreateSingleSnapshot", &(config, description, cleanup, userdata))?;
        let id: u32 = reply.body().deserialize()?;
        Ok(id)
    }

    pub fn delete(&self, config: &str, ids: &[u32]) -> Result<()> {
        let _ = self
            .proxy
            .call_method("DeleteSnapshots", &(config, ids.to_vec()))?;
        Ok(())
    }

    pub fn set_snapshot(
        &self,
        config: &str,
        id: u32,
        description: &str,
        cleanup: &str,
    ) -> Result<()> {
        let userdata: HashMap<String, String> = HashMap::new();
        let _ = self.proxy.call_method(
            "SetSnapshot",
            &(config, id, description, cleanup, userdata),
        )?;
        Ok(())
    }
}

pub fn friendly_error(err: &anyhow::Error) -> String {
    let text = format!("{err:#}");
    if text.contains("error.no_permissions") {
        "permission denied — grant access with the install script (ALLOW_USERS)".into()
    } else if text.contains("error.unknown_config") {
        "unknown snapper config".into()
    } else if text.contains("org.freedesktop.DBus.Error.ServiceUnknown")
        || text.contains("service unknown")
    {
        "snapperd not running — try: systemctl start snapperd".into()
    } else if text.contains("info directory") || text.contains(".snapshots") {
        "no snapshot storage yet — press 'i' to create the .snapshots subvolume".into()
    } else {
        text
    }
}
