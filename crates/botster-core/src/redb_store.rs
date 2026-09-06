//! `redb` backend for the keyed store mechanism.
//!
//! One `redb` table per namespace. Every limit in [`crate::storage`] is
//! validated before a transaction opens. Hosts own the file path and the
//! namespace policy; Core owns the transaction and range semantics.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use redb::{Database, TableDefinition};

use crate::storage::{
    check_batch, check_key, check_range_budget, KeyedStore, Namespace, RangeItem, RangePage,
    StoreError, StoreOp,
};

/// Keyed store persisted in one `redb` database file.
pub struct RedbStore {
    database: Database,
    /// `redb` table names must be `'static`. Each namespace string is leaked
    /// once and reused; the namespace set is small and host-bounded.
    table_names: Mutex<HashMap<Namespace, &'static str>>,
}

impl std::fmt::Debug for RedbStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("RedbStore").finish_non_exhaustive()
    }
}

impl RedbStore {
    /// Open or create the database file at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let database = Database::create(path).map_err(|error| backend("open", error))?;
        Ok(Self {
            database,
            table_names: Mutex::new(HashMap::new()),
        })
    }

    fn table(
        &self,
        namespace: &Namespace,
    ) -> TableDefinition<'static, &'static [u8], &'static [u8]> {
        let mut names = self
            .table_names
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let name = *names
            .entry(namespace.clone())
            .or_insert_with(|| Box::leak(namespace.as_str().to_owned().into_boxed_str()));
        TableDefinition::new(name)
    }
}

fn backend(operation: &'static str, error: impl std::fmt::Display) -> StoreError {
    StoreError::Backend {
        operation,
        message: error.to_string(),
    }
}

impl KeyedStore for RedbStore {
    fn get(&self, namespace: &Namespace, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        check_key(key)?;
        let read = self
            .database
            .begin_read()
            .map_err(|error| backend("get", error))?;
        let table = match read.open_table(self.table(namespace)) {
            Ok(table) => table,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(error) => return Err(backend("get", error)),
        };
        let value = table
            .get(key)
            .map_err(|error| backend("get", error))?
            .map(|guard| guard.value().to_vec());
        Ok(value)
    }

    fn range(
        &self,
        namespace: &Namespace,
        prefix: &[u8],
        after: Option<&[u8]>,
        max_items: usize,
        max_bytes: usize,
    ) -> Result<RangePage, StoreError> {
        check_range_budget(max_items, max_bytes)?;
        if let Some(after) = after {
            check_key(after)?;
        }
        let read = self
            .database
            .begin_read()
            .map_err(|error| backend("range", error))?;
        let table = match read.open_table(self.table(namespace)) {
            Ok(table) => table,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(RangePage::default()),
            Err(error) => return Err(backend("range", error)),
        };
        let start: Vec<u8> = match after {
            Some(after) if after >= prefix => {
                let mut start = after.to_vec();
                start.push(0);
                start
            }
            _ => prefix.to_vec(),
        };
        let mut page = RangePage::default();
        let mut used_bytes = 0usize;
        let iter = table
            .range(start.as_slice()..)
            .map_err(|error| backend("range", error))?;
        for entry in iter {
            let (key, value) = entry.map_err(|error| backend("range", error))?;
            let key = key.value();
            if !key.starts_with(prefix) {
                break;
            }
            let value = value.value();
            let item_bytes = key.len() + value.len();
            if !page.items.is_empty()
                && (page.items.len() >= max_items || used_bytes + item_bytes > max_bytes)
            {
                page.has_more = true;
                break;
            }
            used_bytes += item_bytes;
            page.items.push(RangeItem {
                key: key.to_vec(),
                value: value.to_vec(),
            });
        }
        Ok(page)
    }

    fn batch(&self, namespace: &Namespace, ops: &[StoreOp]) -> Result<(), StoreError> {
        check_batch(ops)?;
        let write = self
            .database
            .begin_write()
            .map_err(|error| backend("batch", error))?;
        {
            let mut table = write
                .open_table(self.table(namespace))
                .map_err(|error| backend("batch", error))?;
            for op in ops {
                match op {
                    StoreOp::Put { key, value } => {
                        table
                            .insert(key.as_slice(), value.as_slice())
                            .map_err(|error| backend("batch", error))?;
                    }
                    StoreOp::Delete { key } => {
                        table
                            .remove(key.as_slice())
                            .map_err(|error| backend("batch", error))?;
                    }
                }
            }
        }
        write.commit().map_err(|error| backend("batch", error))
    }
}
