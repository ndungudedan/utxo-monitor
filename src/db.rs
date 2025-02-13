use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection, PoolError};
use dotenvy::dotenv;
use std::env;
use once_cell::sync::OnceCell;

pub type PgPool = Pool<ConnectionManager<PgConnection>>;

static GLOBAL_POOL: OnceCell<PgPool> = OnceCell::new();

/// Initializes the connection pool only once and returns a reference to it.
pub fn get_pool() -> &'static PgPool {
    GLOBAL_POOL.get_or_init(|| {
        dotenv().ok();

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let manager = ConnectionManager::<PgConnection>::new(database_url);
        Pool::builder()
            .build(manager)
            .expect("Failed to create database connection pool")
    })
}

/// Gets a database connection from the pool.
pub fn get_connection() -> PooledConnection<ConnectionManager<PgConnection>> {
    get_pool().get().unwrap()
}
