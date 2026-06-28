//! Global app state. The DB pool is created once at startup and shared across
//! all axum route handlers. This replaces the Node singleton `getDb()`.

use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::db::DbPool;

static POOL: OnceCell<Arc<DbPool>> = OnceCell::new();

pub fn init_pool(pool: DbPool) {
    let _ = POOL.set(Arc::new(pool));
}

pub fn get_db_pool() -> Arc<DbPool> {
    POOL.get()
        .expect("DB pool not initialized — call init_pool() at startup")
        .clone()
}
