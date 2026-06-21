use crate::api::{
    AppSettingsSnapshot, BootstrapData, ConnectionDraft, ConnectionSession, ConnectionSummary,
    DatabaseKind, QuerySnapshot, TablePreview, TableSummary, ThemeMode, UiLanguage,
};
use crate::cache::SchemaCache;
use crate::db::{init_drivers, ConnectionConfig, DatabaseType, QueryResult, ServerInfo, TableDataResult, TableInfo, Value};
use crate::services::{ConnectionManager, QueryExecutor};
use crate::store::Store;
use anyhow::{anyhow, Context};
use std::sync::{mpsc, Arc};
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};

pub struct BackendGateway {
    sender: tokio_mpsc::UnboundedSender<BackendRequest>,
    thread: Option<thread::JoinHandle<()>>,
}

#[derive(Debug, thiserror::Error)]
pub enum BackendGatewayError {
    #[error("{0}")]
    Message(String),
    #[error("backend request channel is closed")]
    ChannelClosed,
}

enum BackendRequest {
    Bootstrap { reply: oneshot::Sender<anyhow::Result<BootstrapData>> },
    LoadConnection { id: String, reply: oneshot::Sender<anyhow::Result<ConnectionDraft>> },
    SaveConnection { draft: ConnectionDraft, reply: oneshot::Sender<anyhow::Result<ConnectionSummary>> },
    DeleteConnection { id: String, reply: oneshot::Sender<anyhow::Result<()>> },
    TestConnection { draft: ConnectionDraft, reply: oneshot::Sender<anyhow::Result<ServerInfo>> },
    Connect { id: String, reply: oneshot::Sender<anyhow::Result<ConnectionSession>> },
    Disconnect { session_id: String, reply: oneshot::Sender<anyhow::Result<()>> },
    ListDatabases { session_id: String, reply: oneshot::Sender<anyhow::Result<Vec<String>>> },
    ListTables {
        session_id: String,
        database: String,
        reply: oneshot::Sender<anyhow::Result<Vec<TableSummary>>>,
    },
    PreviewTable {
        session_id: String,
        database: String,
        table: String,
        reply: oneshot::Sender<anyhow::Result<TablePreview>>,
    },
    ExecuteQuery {
        session_id: String,
        sql: String,
        reply: oneshot::Sender<anyhow::Result<QuerySnapshot>>,
    },
    SaveSettings {
        language: UiLanguage,
        theme: ThemeMode,
        reply: oneshot::Sender<anyhow::Result<AppSettingsSnapshot>>,
    },
    Shutdown,
}

struct BackendService {
    store: Arc<Store>,
    connection_manager: Arc<ConnectionManager>,
    query_executor: Arc<QueryExecutor>,
}

impl BackendGateway {
    pub fn new() -> Result<Self, BackendGatewayError> {
        let (sender, receiver) = tokio_mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);

        let thread = thread::Builder::new()
            .name("datazen-backend".to_string())
            .spawn(move || {
                let runtime = match Runtime::new() {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = ready_tx.send(Err(anyhow!(error)));
                        return;
                    }
                };

                runtime.block_on(async move {
                    let service = match BackendService::new().await {
                        Ok(service) => {
                            let _ = ready_tx.send(Ok(()));
                            service
                        }
                        Err(error) => {
                            let _ = ready_tx.send(Err(error));
                            return;
                        }
                    };

                    service.run(receiver).await;
                });
            })
            .map_err(|error| BackendGatewayError::Message(error.to_string()))?;

        ready_rx
            .recv()
            .map_err(|_| BackendGatewayError::ChannelClosed)?
            .map_err(|error| BackendGatewayError::Message(error.to_string()))?;

        Ok(Self {
            sender,
            thread: Some(thread),
        })
    }

    pub fn bootstrap(&self) -> Result<BootstrapData, BackendGatewayError> {
        self.request(|reply| BackendRequest::Bootstrap { reply })
    }

    pub fn load_connection(&self, id: impl Into<String>) -> Result<ConnectionDraft, BackendGatewayError> {
        self.request(|reply| BackendRequest::LoadConnection { id: id.into(), reply })
    }

    pub fn save_connection(&self, draft: ConnectionDraft) -> Result<ConnectionSummary, BackendGatewayError> {
        self.request(|reply| BackendRequest::SaveConnection { draft, reply })
    }

    pub fn delete_connection(&self, id: impl Into<String>) -> Result<(), BackendGatewayError> {
        self.request(|reply| BackendRequest::DeleteConnection { id: id.into(), reply })
    }

    pub fn test_connection(&self, draft: ConnectionDraft) -> Result<ServerInfo, BackendGatewayError> {
        self.request(|reply| BackendRequest::TestConnection { draft, reply })
    }

    pub fn connect(&self, id: impl Into<String>) -> Result<ConnectionSession, BackendGatewayError> {
        self.request(|reply| BackendRequest::Connect { id: id.into(), reply })
    }

    pub fn disconnect(&self, session_id: impl Into<String>) -> Result<(), BackendGatewayError> {
        self.request(|reply| BackendRequest::Disconnect { session_id: session_id.into(), reply })
    }

    pub fn list_databases(&self, session_id: impl Into<String>) -> Result<Vec<String>, BackendGatewayError> {
        self.request(|reply| BackendRequest::ListDatabases { session_id: session_id.into(), reply })
    }

    pub fn list_tables(
        &self,
        session_id: impl Into<String>,
        database: impl Into<String>,
    ) -> Result<Vec<TableSummary>, BackendGatewayError> {
        self.request(|reply| BackendRequest::ListTables {
            session_id: session_id.into(),
            database: database.into(),
            reply,
        })
    }

    pub fn preview_table(
        &self,
        session_id: impl Into<String>,
        database: impl Into<String>,
        table: impl Into<String>,
    ) -> Result<TablePreview, BackendGatewayError> {
        self.request(|reply| BackendRequest::PreviewTable {
            session_id: session_id.into(),
            database: database.into(),
            table: table.into(),
            reply,
        })
    }

    pub fn execute_query(
        &self,
        session_id: impl Into<String>,
        sql: impl Into<String>,
    ) -> Result<QuerySnapshot, BackendGatewayError> {
        self.request(|reply| BackendRequest::ExecuteQuery {
            session_id: session_id.into(),
            sql: sql.into(),
            reply,
        })
    }

    pub fn save_settings(
        &self,
        language: UiLanguage,
        theme: ThemeMode,
    ) -> Result<AppSettingsSnapshot, BackendGatewayError> {
        self.request(|reply| BackendRequest::SaveSettings { language, theme, reply })
    }

    fn request<T>(
        &self,
        make_request: impl FnOnce(oneshot::Sender<anyhow::Result<T>>) -> BackendRequest,
    ) -> Result<T, BackendGatewayError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.sender
            .send(make_request(reply_tx))
            .map_err(|_| BackendGatewayError::ChannelClosed)?;

        match reply_rx.blocking_recv() {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(BackendGatewayError::Message(error.to_string())),
            Err(_) => Err(BackendGatewayError::ChannelClosed),
        }
    }
}

impl Drop for BackendGateway {
    fn drop(&mut self) {
        let _ = self.sender.send(BackendRequest::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl BackendService {
    async fn new() -> anyhow::Result<Self> {
        let registry = Arc::new(init_drivers().await);
        let store = Arc::new(Store::init("datazen-native").await?);
        let schema_cache = Arc::new(SchemaCache::new(registry.clone()));
        let connection_manager = Arc::new(ConnectionManager::new(registry, store.clone()));
        connection_manager.clone().start_cleanup_task();

        Ok(Self {
            store,
            connection_manager,
            query_executor: Arc::new(QueryExecutor::new(schema_cache)),
        })
    }

    async fn run(self, mut receiver: tokio_mpsc::UnboundedReceiver<BackendRequest>) {
        while let Some(request) = receiver.recv().await {
            match request {
                BackendRequest::Bootstrap { reply } => {
                    let _ = reply.send(self.bootstrap().await);
                }
                BackendRequest::LoadConnection { id, reply } => {
                    let _ = reply.send(self.load_connection(&id).await);
                }
                BackendRequest::SaveConnection { draft, reply } => {
                    let _ = reply.send(self.save_connection(draft).await);
                }
                BackendRequest::DeleteConnection { id, reply } => {
                    let _ = reply.send(self.delete_connection(&id).await);
                }
                BackendRequest::TestConnection { draft, reply } => {
                    let _ = reply.send(self.test_connection(draft).await);
                }
                BackendRequest::Connect { id, reply } => {
                    let _ = reply.send(self.connect(&id).await);
                }
                BackendRequest::Disconnect { session_id, reply } => {
                    let _ = reply.send(self.disconnect(&session_id).await);
                }
                BackendRequest::ListDatabases { session_id, reply } => {
                    let _ = reply.send(self.list_databases(&session_id).await);
                }
                BackendRequest::ListTables { session_id, database, reply } => {
                    let _ = reply.send(self.list_tables(&session_id, &database).await);
                }
                BackendRequest::PreviewTable { session_id, database, table, reply } => {
                    let _ = reply.send(self.preview_table(&session_id, &database, &table).await);
                }
                BackendRequest::ExecuteQuery { session_id, sql, reply } => {
                    let _ = reply.send(self.execute_query(&session_id, &sql).await);
                }
                BackendRequest::SaveSettings { language, theme, reply } => {
                    let _ = reply.send(self.save_settings(language, theme).await);
                }
                BackendRequest::Shutdown => break,
            }
        }

        self.connection_manager.shutdown().await;
    }

    async fn bootstrap(&self) -> anyhow::Result<BootstrapData> {
        let settings = self.store.get_settings().await;
        let connections = self
            .store
            .get_connections()
            .await
            .into_iter()
            .map(map_connection_summary)
            .collect();

        Ok(BootstrapData {
            settings: AppSettingsSnapshot {
                language: UiLanguage::from_code(&settings.language),
                theme: ThemeMode::from_str(&settings.theme),
            },
            connections,
        })
    }

    async fn load_connection(&self, id: &str) -> anyhow::Result<ConnectionDraft> {
        let config = self
            .store
            .get_connection(id)
            .await
            .with_context(|| format!("connection {id} was not found"))?;
        Ok(map_connection_draft(config))
    }

    async fn save_connection(&self, draft: ConnectionDraft) -> anyhow::Result<ConnectionSummary> {
        let config = build_connection_config(draft)?;
        self.store.save_connection(config.clone()).await?;
        Ok(map_connection_summary(config))
    }

    async fn delete_connection(&self, id: &str) -> anyhow::Result<()> {
        self.store.delete_connection(id).await?;
        Ok(())
    }

    async fn test_connection(&self, draft: ConnectionDraft) -> anyhow::Result<ServerInfo> {
        let config = build_connection_config(draft)?;
        Ok(self.connection_manager.test_connection(&config).await?)
    }

    async fn connect(&self, id: &str) -> anyhow::Result<ConnectionSession> {
        let session_id = self.connection_manager.connect(id).await?;
        let config = self
            .store
            .get_connection(id)
            .await
            .with_context(|| format!("connection {id} was not found"))?;
        let server = self.connection_manager.test_connection(&config).await?;

        let mut updated = config;
        updated.last_connected_at = Some(chrono::Utc::now().to_rfc3339());
        self.store.save_connection(updated).await?;

        Ok(ConnectionSession {
            session_id,
            server_type: server.server_type,
            server_version: server.server_version,
        })
    }

    async fn disconnect(&self, session_id: &str) -> anyhow::Result<()> {
        self.connection_manager.disconnect(session_id).await?;
        Ok(())
    }

    async fn list_databases(&self, session_id: &str) -> anyhow::Result<Vec<String>> {
        let (driver, handle) = self.connection_manager.get_connection(session_id).await?;
        Ok(driver.get_databases(&handle).await?)
    }

    async fn list_tables(&self, session_id: &str, database: &str) -> anyhow::Result<Vec<TableSummary>> {
        let (driver, handle) = self.connection_manager.get_connection(session_id).await?;
        Ok(driver
            .get_tables(&handle, database)
            .await?
            .into_iter()
            .map(map_table_summary)
            .collect())
    }

    async fn preview_table(&self, session_id: &str, database: &str, table: &str) -> anyhow::Result<TablePreview> {
        let (driver, handle) = self.connection_manager.get_connection(session_id).await?;
        let data = self
            .query_executor
            .get_table_data(&driver, &handle, session_id, database, table, 0, 100, None, None, false)
            .await?;
        Ok(map_table_preview(data))
    }

    async fn execute_query(&self, session_id: &str, sql: &str) -> anyhow::Result<QuerySnapshot> {
        let (driver, handle) = self.connection_manager.get_connection(session_id).await?;
        let result = self.query_executor.execute_query(&driver, &handle, sql).await?;
        Ok(map_query_snapshot(result))
    }

    async fn save_settings(&self, language: UiLanguage, theme: ThemeMode) -> anyhow::Result<AppSettingsSnapshot> {
        let mut settings = self.store.get_settings().await;
        settings.language = language.as_code().to_string();
        settings.theme = theme.as_str().to_string();
        self.store.save_settings(settings.clone()).await?;
        Ok(AppSettingsSnapshot {
            language: UiLanguage::from_code(&settings.language),
            theme: ThemeMode::from_str(&settings.theme),
        })
    }
}

fn build_connection_config(draft: ConnectionDraft) -> anyhow::Result<ConnectionConfig> {
    let name = draft.name.trim();
    if name.is_empty() {
        anyhow::bail!("connection name is required");
    }

    Ok(ConnectionConfig {
        id: draft.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        name: name.to_string(),
        database_type: map_database_kind(draft.database_kind),
        host: sanitize_optional(draft.host),
        port: draft.port,
        database: sanitize_optional(draft.database),
        username: sanitize_optional(draft.username),
        password: sanitize_optional(draft.password),
        ssl_mode: Default::default(),
        connection_timeout: 30,
        ssh_tunnel: None,
        color_tag: sanitize_optional(draft.color_tag),
        group: sanitize_optional(draft.group),
        last_connected_at: None,
    })
}

fn sanitize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn map_database_kind(kind: DatabaseKind) -> DatabaseType {
    match kind {
        DatabaseKind::PostgreSql => DatabaseType::PostgreSQL,
        DatabaseKind::MySql => DatabaseType::MySQL,
        DatabaseKind::MariaDb => DatabaseType::MariaDB,
        DatabaseKind::Sqlite => DatabaseType::SQLite,
        DatabaseKind::Redis => DatabaseType::Redis,
    }
}

fn map_database_type(kind: DatabaseType) -> DatabaseKind {
    match kind {
        DatabaseType::PostgreSQL => DatabaseKind::PostgreSql,
        DatabaseType::MySQL => DatabaseKind::MySql,
        DatabaseType::MariaDB => DatabaseKind::MariaDb,
        DatabaseType::SQLite => DatabaseKind::Sqlite,
        DatabaseType::Redis => DatabaseKind::Redis,
    }
}

fn map_connection_draft(config: ConnectionConfig) -> ConnectionDraft {
    ConnectionDraft {
        id: Some(config.id),
        name: config.name,
        database_kind: map_database_type(config.database_type),
        host: config.host,
        port: config.port,
        database: config.database,
        username: config.username,
        password: config.password,
        group: config.group,
        color_tag: config.color_tag,
    }
}

fn map_connection_summary(config: ConnectionConfig) -> ConnectionSummary {
    ConnectionSummary {
        id: config.id,
        name: config.name,
        database_kind: map_database_type(config.database_type),
        host: config.host,
        database: config.database,
        group: config.group,
        color_tag: config.color_tag,
        last_connected_at: config.last_connected_at,
    }
}

fn map_table_summary(table: TableInfo) -> TableSummary {
    TableSummary {
        name: table.name,
        table_type: format!("{:?}", table.table_type).to_lowercase(),
        row_count: table.row_count,
    }
}

fn map_query_snapshot(result: QueryResult) -> QuerySnapshot {
    QuerySnapshot {
        columns: result.columns.into_iter().map(|column| column.name).collect(),
        rows: result
            .rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|value| value.map(render_value).unwrap_or_else(|| "NULL".to_string()))
                    .collect()
            })
            .collect(),
        rows_affected: result.rows_affected,
        execution_time_ms: result.execution_time_ms,
    }
}

fn map_table_preview(data: TableDataResult) -> TablePreview {
    TablePreview {
        columns: data.columns.into_iter().map(|column| column.name).collect(),
        rows: data
            .rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|value| value.map(render_value).unwrap_or_else(|| "NULL".to_string()))
                    .collect()
            })
            .collect(),
        total_rows: data.total_rows,
        page: data.page,
        page_size: data.page_size,
    }
}

fn render_value(value: Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Integer(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::String(value) => value,
        Value::Bytes(value) => format!("<{} bytes>", value.len()),
        Value::Timestamp(value) => value,
        Value::Json(value) => value.to_string(),
    }
}
