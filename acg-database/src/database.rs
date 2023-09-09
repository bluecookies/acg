use std::sync::Arc;
use deadpool_postgres::{BuildError, Manager, Pool as PostgresPool, ManagerConfig, RecyclingMethod, Object};
use tokio_postgres::{Config as PostgresConfig, NoTls};
use crate::Error;

#[derive(Clone)]
pub struct Database {
    inner: DatabaseInner,
}

#[derive(Clone)]
enum DatabaseInner {
    Pool(PostgresPool),
    ConfigError(Arc<str>),
    NoRuntimeSpecified(Arc<str>),
    Error(Arc<str>),
    // this case should be impossible
    BuildError(Arc<str>),
}

impl Database {
    pub fn new<S: AsRef<str>>(connection_string: S) -> Self {
        let inner = match connection_string.as_ref().parse::<PostgresConfig>() {
            Err(e) => DatabaseInner::ConfigError(e.to_string().into()),
            Ok(pg_config) => {
                let mgr_config = ManagerConfig {
                    recycling_method: RecyclingMethod::Fast,
                };
                let manager = Manager::from_config(pg_config, NoTls, mgr_config);
                match PostgresPool::builder(manager).build() {
                    Ok(v) => DatabaseInner::Pool(v),
                    Err(BuildError::NoRuntimeSpecified(e)) => DatabaseInner::NoRuntimeSpecified(e.into()),
                    Err(BuildError::Backend(e)) => DatabaseInner::BuildError(e.to_string().into()),
                }
            }
        };

        match inner {
            DatabaseInner::Pool(_) => {},
            DatabaseInner::ConfigError(ref s) |
            DatabaseInner::NoRuntimeSpecified(ref s) |
            DatabaseInner::BuildError(ref s) |
            DatabaseInner::Error(ref s) => log::error!("failed to init database connection: {}", s),
        }

        Database {
            inner,
        }
    }

    pub fn error(s: &'static str) -> Self {
        Database {
            inner: DatabaseInner::Error(s.into()),
        }
    }

    pub async fn client(&self) -> Result<Object, Error> {
        match self.inner {
            DatabaseInner::Pool(ref p) => Ok(p.get().await?),
            _ => Err(Error::NoDatabase),
        }
    }
}


