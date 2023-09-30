use crate::{Database, Error};
use tokio_postgres::types::ToSql;
use tokio_postgres::Row;

impl Database {
    pub async fn get_stats<T>(
        &self,
        query: &str,
        name: &'static str,
        map: impl Fn(Row) -> T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<T>, Error> {
        let client = self.client().await?;

        let statement = prepare_statement!(client, query, name)?;
        let rows = client.query(&statement, params).await?;
        let result = rows.into_iter().map(map).collect();
        Ok(result)
    }
}
