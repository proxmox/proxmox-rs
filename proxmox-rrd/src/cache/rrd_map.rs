use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Error};

use proxmox_sys::fs::create_path;

use crate::rrd::{AggregationFn, DataSourceType, Database};

use super::CacheConfig;
use crate::Entry;

pub struct RRDMap {
    config: Arc<CacheConfig>,
    map: HashMap<String, Database>,
    load_rrd_cb: fn(path: &Path, rel_path: &str) -> Option<Database>,
    create_rrd_cb: fn(dst: DataSourceType) -> Database,
}

impl RRDMap {
    pub(crate) fn new(
        config: Arc<CacheConfig>,
        load_rrd_cb: fn(path: &Path, rel_path: &str) -> Option<Database>,
        create_rrd_cb: fn(dst: DataSourceType) -> Database,
    ) -> Self {
        Self {
            config,
            map: HashMap::new(),
            load_rrd_cb,
            create_rrd_cb,
        }
    }

    pub fn update(
        &mut self,
        rel_path: &str,
        time: f64,
        value: f64,
        dst: DataSourceType,
        new_only: bool,
    ) -> Result<(), Error> {
        if let Some(rrd) = self.map.get_mut(rel_path) {
            if !new_only || time > rrd.last_update() {
                rrd.update(time, value);
            }
        } else {
            let mut path = self.config.basedir.clone();
            path.push(rel_path);
            let mut rrd = match (self.load_rrd_cb)(&path, rel_path) {
                None => {
                    create_path(
                        path.parent().unwrap(),
                        Some(self.config.dir_options),
                        Some(self.config.dir_options),
                    )?;

                    (self.create_rrd_cb)(dst)
                }
                Some(rrd) => rrd,
            };

            if !new_only || time > rrd.last_update() {
                rrd.update(time, value);
            }
            self.map.insert(rel_path.to_string(), rrd);
        }
        Ok(())
    }

    pub fn file_list(&self) -> Vec<String> {
        let mut list = Vec::new();

        for rel_path in self.map.keys() {
            list.push(rel_path.clone());
        }

        list
    }

    pub fn flush_rrd_file(&self, rel_path: &str) -> Result<(), Error> {
        if let Some(rrd) = self.map.get(rel_path) {
            let mut path = self.config.basedir.clone();
            path.push(rel_path);
            rrd.save(&path, self.config.file_options, true)
        } else {
            bail!("rrd file {} not loaded", rel_path);
        }
    }

    pub fn extract_cached_data(
        &self,
        base: &str,
        name: &str,
        cf: AggregationFn,
        resolution: u64,
        start: Option<u64>,
        end: Option<u64>,
    ) -> Result<Option<Entry>, Error> {
        match self.map.get(&format!("{}/{}", base, name)) {
            Some(rrd) => Ok(Some(rrd.extract_data(cf, resolution, start, end)?)),
            None => Ok(None),
        }
    }

    pub fn load(&mut self, rel_path: &str) -> Result<bool, Error> {
        if self.map.contains_key(rel_path) {
            // Already loaded, do nothing
            return Ok(true);
        }

        let mut path = self.config.basedir.clone();
        path.push(rel_path);

        if let Some(rrd) = (self.load_rrd_cb)(&path, rel_path) {
            self.map.insert(rel_path.to_string(), rrd);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
