use std::{collections::BTreeMap, sync::Arc};

use anyhow::Result;
use backend::{
    BackendGateway, BootstrapData, ConnectionDraft, ConnectionSession, ConnectionSummary,
    DatabaseKind, QuerySnapshot, TablePreview, TableSummary, ThemeMode, UiLanguage,
};
use gpui::{
    App, AppContext, Application, Bounds, Context, Entity, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, SharedString, Styled, Window, WindowBounds,
    WindowOptions, div, px, rgb, size, white,
    prelude::FluentBuilder,
};

use crate::i18n::{ALL_LANGUAGES, I18n};
use crate::input::{TextInput, bind_input_keys};

const BACKGROUND: u32 = 0x0a1016;
const RAIL: u32 = 0x0e151d;
const SURFACE: u32 = 0x111923;
const SURFACE_ALT: u32 = 0x182230;
const SURFACE_ELEVATED: u32 = 0x1d2a39;
const BORDER: u32 = 0x27384b;
const BORDER_SOFT: u32 = 0x31465c;
const TEXT_MUTED: u32 = 0x8aa0b4;
const TEXT_SOFT: u32 = 0x5f758b;
const ACCENT: u32 = 0x72adff;
const ACCENT_SOFT: u32 = 0x1a3558;
const SUCCESS: u32 = 0x39d39f;
const WARNING: u32 = 0xd3b36b;
const DANGER: u32 = 0xcc6d6d;
const TERMINAL_BG: u32 = 0x08111a;
const TERMINAL_EDGE: u32 = 0x203246;
const TERMINAL_GREEN: u32 = 0x91f4cb;
const TERMINAL_AMBER: u32 = 0xf0c879;

struct ConnectionForm {
    id: Option<String>,
    database_kind: DatabaseKind,
    name: Entity<TextInput>,
    host: Entity<TextInput>,
    port: Entity<TextInput>,
    database: Entity<TextInput>,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    group: Entity<TextInput>,
    color_tag: Entity<TextInput>,
}

impl ConnectionForm {
    fn new(cx: &mut App) -> Self {
        Self {
            id: None,
            database_kind: DatabaseKind::PostgreSql,
            name: TextInput::new("", "Primary connection", false, cx),
            host: TextInput::new("", "localhost", false, cx),
            port: TextInput::new("", "5432", false, cx),
            database: TextInput::new("", "app_db", false, cx),
            username: TextInput::new("", "postgres", false, cx),
            password: TextInput::new("", "password", true, cx),
            group: TextInput::new("", "Core", false, cx),
            color_tag: TextInput::new("", "blue", false, cx),
        }
    }

    fn clear(&mut self, cx: &mut Context<RootView>) {
        self.id = None;
        self.database_kind = DatabaseKind::PostgreSql;
        self.name.update(cx, |field, cx| field.set_text("", cx));
        self.host.update(cx, |field, cx| field.set_text("", cx));
        self.port.update(cx, |field, cx| field.set_text("", cx));
        self.database.update(cx, |field, cx| field.set_text("", cx));
        self.username.update(cx, |field, cx| field.set_text("", cx));
        self.password.update(cx, |field, cx| field.set_text("", cx));
        self.group.update(cx, |field, cx| field.set_text("", cx));
        self.color_tag.update(cx, |field, cx| field.set_text("", cx));
    }

    fn populate(&mut self, draft: ConnectionDraft, cx: &mut Context<RootView>) {
        self.id = draft.id;
        self.database_kind = draft.database_kind;
        self.name
            .update(cx, |field, cx| field.set_text(draft.name, cx));
        self.host
            .update(cx, |field, cx| field.set_text(draft.host.unwrap_or_default(), cx));
        self.port.update(cx, |field, cx| {
            field.set_text(draft.port.map(|value| value.to_string()).unwrap_or_default(), cx)
        });
        self.database
            .update(cx, |field, cx| field.set_text(draft.database.unwrap_or_default(), cx));
        self.username
            .update(cx, |field, cx| field.set_text(draft.username.unwrap_or_default(), cx));
        self.password
            .update(cx, |field, cx| field.set_text(draft.password.unwrap_or_default(), cx));
        self.group
            .update(cx, |field, cx| field.set_text(draft.group.unwrap_or_default(), cx));
        self.color_tag
            .update(cx, |field, cx| field.set_text(draft.color_tag.unwrap_or_default(), cx));
    }

    fn to_draft(&self, cx: &App) -> ConnectionDraft {
        ConnectionDraft {
            id: self.id.clone(),
            name: self.name.read(cx).text(),
            database_kind: self.database_kind,
            host: optional_string(self.host.read(cx).text()),
            port: optional_string(self.port.read(cx).text()).and_then(|value| value.parse().ok()),
            database: optional_string(self.database.read(cx).text()),
            username: optional_string(self.username.read(cx).text()),
            password: optional_string(self.password.read(cx).text()),
            group: optional_string(self.group.read(cx).text()),
            color_tag: optional_string(self.color_tag.read(cx).text()),
        }
    }
}

struct RootView {
    gateway: Arc<BackendGateway>,
    i18n: I18n,
    theme: ThemeMode,
    query_input: Entity<TextInput>,
    connection_filter: Entity<TextInput>,
    form: ConnectionForm,
    connections: Vec<ConnectionSummary>,
    selected_connection_id: Option<String>,
    session: Option<ConnectionSession>,
    databases: Vec<String>,
    selected_database: Option<String>,
    tables: Vec<TableSummary>,
    selected_table: Option<String>,
    table_preview: Option<TablePreview>,
    query_result: Option<QuerySnapshot>,
    show_form: bool,
    show_language_menu: bool,
    busy_label: Option<SharedString>,
    status: SharedString,
    error: Option<SharedString>,
}

impl RootView {
    fn sync_locale_inputs(&mut self, cx: &mut Context<Self>) {
        self.connection_filter.update(cx, |field, cx| {
            field.set_placeholder(self.i18n.tr("sidebar.search_placeholder"), cx);
        });
        self.query_input.update(cx, |field, cx| {
            field.set_placeholder(self.i18n.tr("terminal.placeholder"), cx);
        });
    }

    fn on_reload(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.bootstrap(cx);
    }

    fn on_new_connection(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.form.clear(cx);
        self.show_form = true;
        self.error = None;
        cx.notify();
    }

    fn on_cancel_form(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_form = false;
        cx.notify();
    }

    fn on_toggle_language_menu(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_language_menu = !self.show_language_menu;
        cx.notify();
    }

    fn on_save_connection(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft = self.form.to_draft(cx);
        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.saving").into());
        self.error = None;
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.save_connection(draft) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(connection) => {
                    view.upsert_connection(connection);
                    view.show_form = false;
                    view.status = view.i18n.tr("status.ready").into();
                    view.busy_label = None;
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn on_test_connection(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let draft = self.form.to_draft(cx);
        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.testing").into());
        self.error = None;
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.test_connection(draft) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(server) => {
                    view.status = format!(
                        "{}: {} {}",
                        view.i18n.tr("status.connected"),
                        server.server_type,
                        server.server_version
                    )
                    .into();
                    view.busy_label = None;
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn on_edit_selected(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.selected_connection_id.clone() else {
            return;
        };

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.loading").into());
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.load_connection(id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(draft) => {
                    view.form.populate(draft, cx);
                    view.show_form = true;
                    view.busy_label = None;
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn on_delete_selected(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.selected_connection_id.clone() else {
            return;
        };

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.loading").into());
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.delete_connection(id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(()) => {
                    if let Some(selected) = &view.selected_connection_id {
                        view.connections.retain(|connection| &connection.id != selected);
                    }
                    view.selected_connection_id =
                        view.connections.first().map(|connection| connection.id.clone());
                    view.session = None;
                    view.databases.clear();
                    view.tables.clear();
                    view.selected_database = None;
                    view.selected_table = None;
                    view.table_preview = None;
                    view.query_result = None;
                    view.busy_label = None;
                    view.status = view.i18n.tr("status.ready").into();
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn on_connect_selected(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.selected_connection_id.clone() else {
            return;
        };

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.connecting").into());
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.connect(id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(session) => {
                    view.status = format!(
                        "{}: {} {}",
                        view.i18n.tr("status.connected"),
                        session.server_type,
                        session.server_version
                    )
                    .into();
                    view.session = Some(session);
                    view.busy_label = None;
                    view.error = None;
                    cx.notify();
                    view.load_databases(cx);
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn on_disconnect(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session.clone() else {
            return;
        };

        let gateway = self.gateway.clone();
        let task = cx
            .background_executor()
            .spawn(async move { gateway.disconnect(session.session_id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(()) => {
                    view.session = None;
                    view.databases.clear();
                    view.tables.clear();
                    view.selected_database = None;
                    view.selected_table = None;
                    view.table_preview = None;
                    view.query_result = None;
                    view.busy_label = None;
                    view.status = view.i18n.tr("status.disconnected").into();
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn load_databases(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.loading").into());
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.list_databases(session.session_id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(databases) => {
                    view.databases = databases;
                    view.selected_database = view.databases.first().cloned();
                    view.tables.clear();
                    view.selected_table = None;
                    view.table_preview = None;
                    view.busy_label = None;
                    cx.notify();
                    view.load_tables(cx);
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn load_tables(&mut self, cx: &mut Context<Self>) {
        let (Some(session), Some(database)) =
            (self.session.clone(), self.selected_database.clone())
        else {
            return;
        };

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.loading").into());
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.list_tables(session.session_id, database) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(tables) => {
                    view.tables = tables;
                    view.selected_table = view.tables.first().map(|table| table.name.clone());
                    view.busy_label = None;
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn on_preview_table(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(session), Some(database), Some(table)) = (
            self.session.clone(),
            self.selected_database.clone(),
            self.selected_table.clone(),
        ) else {
            return;
        };

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.loading").into());
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.preview_table(session.session_id, database, table) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(preview) => {
                    view.table_preview = Some(preview);
                    view.busy_label = None;
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn on_run_query(
        &mut self,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session.clone() else {
            return;
        };
        let sql = self.query_input.read(cx).text();
        if sql.trim().is_empty() {
            return;
        }

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.loading").into());
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.execute_query(session.session_id, sql) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(snapshot) => {
                    view.query_result = Some(snapshot);
                    view.busy_label = None;
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn cycle_language(
        &mut self,
        language: UiLanguage,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let gateway = self.gateway.clone();
        let theme = self.theme;
        self.busy_label = Some(self.i18n.tr("status.saving").into());
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { gateway.save_settings(language, theme) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(settings) => {
                    view.i18n.set_language(settings.language);
                    view.theme = settings.theme;
                    view.sync_locale_inputs(cx);
                    view.show_language_menu = false;
                    view.busy_label = None;
                    view.status = view.i18n.tr("status.ready").into();
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn select_connection(&mut self, id: String, cx: &mut Context<Self>) {
        self.selected_connection_id = Some(id);
        self.session = None;
        self.databases.clear();
        self.tables.clear();
        self.selected_database = None;
        self.selected_table = None;
        self.table_preview = None;
        self.query_result = None;
        self.error = None;
        cx.notify();
    }

    fn select_database(
        &mut self,
        database: String,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_database = Some(database);
        self.tables.clear();
        self.selected_table = None;
        self.table_preview = None;
        cx.notify();
        self.load_tables(cx);
    }

    fn select_table(&mut self, table: String, cx: &mut Context<Self>) {
        self.selected_table = Some(table);
        self.table_preview = None;
        cx.notify();
    }

    fn upsert_connection(&mut self, connection: ConnectionSummary) {
        if let Some(existing) = self
            .connections
            .iter_mut()
            .find(|item| item.id == connection.id)
        {
            *existing = connection.clone();
        } else {
            self.connections.push(connection.clone());
        }
        self.connections.sort_by(|left, right| left.name.cmp(&right.name));
        self.selected_connection_id = Some(connection.id);
    }

    fn bootstrap(&mut self, cx: &mut Context<Self>) {
        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.loading").into());
        self.error = None;
        cx.notify();

        let task = cx.background_executor().spawn(async move { gateway.bootstrap() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                Ok(data) => {
                    view.apply_bootstrap(data);
                    view.sync_locale_inputs(cx);
                    view.busy_label = None;
                    cx.notify();
                }
                Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn apply_bootstrap(&mut self, data: BootstrapData) {
        self.connections = data.connections;
        self.selected_connection_id =
            self.connections.first().map(|connection| connection.id.clone());
        self.i18n.set_language(data.settings.language);
        self.theme = data.settings.theme;
        self.status = self.i18n.tr("status.ready").into();
    }

    fn set_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.error = Some(message.into());
        self.busy_label = None;
        self.status = self.i18n.tr("status.ready").into();
        cx.notify();
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status_label = self
            .busy_label
            .clone()
            .unwrap_or_else(|| self.status.clone());

        let selected_connection = self
            .selected_connection_id
            .as_ref()
            .and_then(|id| self.connections.iter().find(|connection| &connection.id == id))
            .cloned();

        let filter = self.connection_filter.read(cx).text().to_lowercase();
        let grouped_connections = group_connections(
            self.connections
                .iter()
                .filter(|connection| {
                    filter.is_empty()
                        || connection.name.to_lowercase().contains(&filter)
                        || connection
                            .host
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&filter)
                        || connection
                            .database
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&filter)
                        || connection
                            .group
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&filter)
                })
                .cloned()
                .collect(),
            self.i18n.tr("workspace.environment"),
        );

        let connected_label = selected_connection
            .as_ref()
            .map(connection_identity)
            .unwrap_or_else(|| self.i18n.tr("workspace.pick_connection").to_string());

        let environment_label = selected_connection
            .as_ref()
            .and_then(|connection| connection.group.clone())
            .unwrap_or_else(|| self.i18n.tr("workspace.environment").to_string());

        div()
            .relative()
            .size_full()
            .bg(rgb(BACKGROUND))
            .text_color(white())
            .child(
                div()
                    .flex()
                    .size_full()
                    .child(
                        div()
                            .w(px(388.))
                            .border_r_1()
                            .border_color(rgb(BORDER))
                            .flex()
                            .child(
                                div()
                                    .w(px(76.))
                                    .bg(rgb(RAIL))
                                    .p_3()
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_3()
                                            .child(brand_mark())
                                            .child(
                                                rail_button("＋", self.i18n.tr("actions.new"))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(Self::on_new_connection),
                                                    ),
                                            )
                                            .child(
                                                rail_button("↻", self.i18n.tr("actions.reload"))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(Self::on_reload),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .size_9()
                                                    .rounded_full()
                                                    .bg(if self.session.is_some() {
                                                        rgb(SUCCESS)
                                                    } else {
                                                        rgb(TEXT_SOFT)
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(TEXT_SOFT))
                                                    .child(self.i18n.tr("selector.language")),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .bg(rgb(SURFACE))
                                    .p_4()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .child(self.i18n.tr("workspace.connection")),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(rgb(TEXT_MUTED))
                                                    .child(self.i18n.tr("app.subtitle")),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(TEXT_SOFT))
                                                    .child(self.i18n.tr("sidebar.search")),
                                            )
                                            .child(self.connection_filter.clone()),
                                    )
                                    .when(grouped_connections.is_empty(), |panel| {
                                        panel.child(sidebar_empty_state(
                                            self.i18n.tr("sidebar.no_connections"),
                                            self.i18n.tr("sidebar.empty_hint"),
                                            self.i18n.tr("sidebar.supported"),
                                        ))
                                        .child(
                                            sidebar_quickstart(
                                                self.i18n.tr("sidebar.quick_start"),
                                                self.i18n.tr("sidebar.templates_hint"),
                                            )
                                            .child(
                                                quick_start_button(
                                                    "Pg",
                                                    self.i18n.tr("actions.create_postgres"),
                                                    db_kind_color(DatabaseKind::PostgreSql),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.form.clear(cx);
                                                        view.form.database_kind =
                                                            DatabaseKind::PostgreSql;
                                                        view.show_form = true;
                                                        cx.notify();
                                                    }),
                                                ),
                                            )
                                            .child(
                                                quick_start_button(
                                                    "My",
                                                    self.i18n.tr("actions.create_mysql"),
                                                    db_kind_color(DatabaseKind::MySql),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.form.clear(cx);
                                                        view.form.database_kind =
                                                            DatabaseKind::MySql;
                                                        view.show_form = true;
                                                        cx.notify();
                                                    }),
                                                ),
                                            )
                                            .child(
                                                quick_start_button(
                                                    "Sq",
                                                    self.i18n.tr("actions.create_sqlite"),
                                                    db_kind_color(DatabaseKind::Sqlite),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.form.clear(cx);
                                                        view.form.database_kind =
                                                            DatabaseKind::Sqlite;
                                                        view.show_form = true;
                                                        cx.notify();
                                                    }),
                                                ),
                                            )
                                            .child(
                                                quick_start_button(
                                                    "Rd",
                                                    self.i18n.tr("actions.create_redis"),
                                                    db_kind_color(DatabaseKind::Redis),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|view, _, _, cx| {
                                                        view.form.clear(cx);
                                                        view.form.database_kind =
                                                            DatabaseKind::Redis;
                                                        view.show_form = true;
                                                        cx.notify();
                                                    }),
                                                ),
                                            ),
                                        )
                                    })
                                    .children(grouped_connections.into_iter().map(
                                        |(group, connections)| {
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .child(group_label(group))
                                                .children(connections.into_iter().map(|connection| {
                                                    let selected = self
                                                        .selected_connection_id
                                                        .as_ref()
                                                        .map(|id| id == &connection.id)
                                                        .unwrap_or(false);
                                                    let id = connection.id.clone();
                                                    connection_card(&connection, selected).on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(move |view, _, _, cx| {
                                                            view.select_connection(id.clone(), cx);
                                                        }),
                                                    )
                                                }))
                                        },
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .rounded_xl()
                                    .border_1()
                                    .border_color(rgb(BORDER))
                                    .bg(rgb(SURFACE))
                                    .px_4()
                                    .py_3()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_3()
                                                    .child(logo_badge())
                                                    .child(
                                                        div()
                                                            .text_xl()
                                                            .font_weight(gpui::FontWeight::BOLD)
                                                            .child(
                                                                selected_connection
                                                                    .as_ref()
                                                                    .map(|connection| {
                                                                        connection.name.clone()
                                                                    })
                                                                    .unwrap_or_else(|| {
                                                                        self.i18n
                                                                            .tr("app.title")
                                                                            .to_string()
                                                                    }),
                                                            ),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap_2()
                                                    .flex_wrap()
                                                    .child(meta_pill(
                                                        "db",
                                                        selected_connection
                                                            .as_ref()
                                                            .map(|connection| {
                                                                db_kind_name(
                                                                    connection.database_kind,
                                                                )
                                                            })
                                                            .unwrap_or(""),
                                                        ACCENT_SOFT,
                                                        ACCENT,
                                                    ))
                                                    .child(meta_pill(
                                                        "⟷",
                                                        &connected_label,
                                                        SURFACE_ALT,
                                                        TEXT_MUTED,
                                                    ))
                                                    .child(meta_pill(
                                                        "⌂",
                                                        &environment_label,
                                                        SURFACE_ALT,
                                                        TEXT_MUTED,
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                subtle_button(
                                                    "◎",
                                                    format!(
                                                        "{} · {}",
                                                        self.i18n.tr("selector.language"),
                                                        self.i18n.language_label(
                                                            self.i18n.language()
                                                        )
                                                    ),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(Self::on_toggle_language_menu),
                                                ),
                                            )
                                            .child(
                                                subtle_button("✎", self.i18n.tr("actions.edit"))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(Self::on_edit_selected),
                                                    ),
                                            )
                                            .child(
                                                danger_button(
                                                    "⌫",
                                                    self.i18n.tr("actions.delete"),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(Self::on_delete_selected),
                                                ),
                                            )
                                            .child(if self.session.is_some() {
                                                accent_button(
                                                    "⨯",
                                                    self.i18n.tr("actions.disconnect"),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(Self::on_disconnect),
                                                )
                                                .into_any_element()
                                            } else {
                                                accent_button(
                                                    "⇄",
                                                    self.i18n.tr("actions.connect"),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(Self::on_connect_selected),
                                                )
                                                .into_any_element()
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(320.))
                                            .flex()
                                            .flex_col()
                                            .gap_3()
                                            .child(
                                                workspace_panel(self.i18n.tr("workspace.databases"))
                                                    .child(if self.databases.is_empty() {
                                                        empty_notice(if self.session.is_some() {
                                                            self.i18n.tr("status.loading")
                                                        } else {
                                                            self.i18n.tr("workspace.not_connected")
                                                        })
                                                        .into_any_element()
                                                    } else {
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_2()
                                                            .children(
                                                                self.databases
                                                                    .iter()
                                                                    .map(|database| {
                                                                        let selected =
                                                                            self.selected_database
                                                                                .as_ref()
                                                                                == Some(database);
                                                                        let db =
                                                                            database.clone();
                                                                        list_row(
                                                                            "▣",
                                                                            database,
                                                                            "",
                                                                            selected,
                                                                        )
                                                                        .on_mouse_up(
                                                                            MouseButton::Left,
                                                                            cx.listener(
                                                                                move |view, _, window, cx| {
                                                                                    view.select_database(
                                                                                        db.clone(),
                                                                                        window,
                                                                                        cx,
                                                                                    );
                                                                                },
                                                                            ),
                                                                        )
                                                                    }),
                                                            )
                                                            .into_any_element()
                                                    }),
                                            )
                                            .child(
                                                workspace_panel_with_action(
                                                    self.i18n.tr("workspace.tables"),
                                                    self.i18n.tr("actions.open_preview"),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(Self::on_preview_table),
                                                )
                                                .child(if self.tables.is_empty() {
                                                    empty_notice(self.i18n.tr("workspace.not_connected"))
                                                        .into_any_element()
                                                } else {
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_2()
                                                        .children(
                                                            self.tables.iter().map(|table| {
                                                                let selected =
                                                                    self.selected_table.as_ref()
                                                                        == Some(&table.name);
                                                                let name = table.name.clone();
                                                                let detail = table
                                                                    .row_count
                                                                    .map(|rows| rows.to_string())
                                                                    .unwrap_or_else(|| {
                                                                        table.table_type.clone()
                                                                    });
                                                                list_row(
                                                                    "≡",
                                                                    &table.name,
                                                                    detail,
                                                                    selected,
                                                                )
                                                                .on_mouse_up(
                                                                    MouseButton::Left,
                                                                    cx.listener(move |view, _, _, cx| {
                                                                        view.select_table(
                                                                            name.clone(),
                                                                            cx,
                                                                        );
                                                                    }),
                                                                )
                                                            }),
                                                        )
                                                        .into_any_element()
                                                }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_3()
                                            .child(
                                                workspace_panel(self.i18n.tr("workspace.preview"))
                                                    .child(render_preview(
                                                        self.table_preview.as_ref(),
                                                        self.i18n.tr("workspace.sample"),
                                                        self.i18n.tr("workspace.not_connected"),
                                                        self.i18n.tr("stats.columns"),
                                                        self.i18n.tr("stats.rows"),
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                terminal_panel(
                                    self.i18n.tr("terminal.title"),
                                    self.i18n.tr("terminal.helper"),
                                    connected_label,
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .child(
                                            div()
                                                .w(px(44.))
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(TERMINAL_GREEN))
                                                .child(format!(
                                                    "{} >",
                                                    self.i18n.tr("terminal.prompt")
                                                )),
                                        )
                                        .child(div().flex_1().child(self.query_input.clone()))
                                        .child(
                                            accent_button(
                                                "▶",
                                                self.i18n.tr("actions.run_query"),
                                            )
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::on_run_query),
                                            ),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(TEXT_SOFT))
                                        .child(self.i18n.tr("terminal.ready_line")),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(TEXT_MUTED))
                                        .child(self.i18n.tr("terminal.results")),
                                )
                                .child(render_query_terminal(
                                    self.query_result.as_ref(),
                                    self.i18n.tr("terminal.empty"),
                                    self.i18n.tr("stats.rows"),
                                    self.i18n.tr("stats.columns"),
                                )),
                            )
                            .child(
                                div()
                                    .h(px(36.))
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(rgb(BORDER))
                                    .bg(rgb(SURFACE))
                                    .px_3()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(TEXT_MUTED))
                                            .child(status_label),
                                    )
                                    .child(
                                        self.error
                                            .as_ref()
                                            .map(|error| {
                                                div()
                                                    .text_sm()
                                                    .text_color(rgb(DANGER))
                                                    .child(error.clone())
                                                    .into_any_element()
                                            })
                                            .unwrap_or_else(|| {
                                                div()
                                                    .text_sm()
                                                    .text_color(if self.session.is_some() {
                                                        rgb(SUCCESS)
                                                    } else {
                                                        rgb(TEXT_MUTED)
                                                    })
                                                    .child(if self.session.is_some() {
                                                        self.i18n.tr("status.connected")
                                                    } else {
                                                        self.i18n.tr("status.ready")
                                                    })
                                                    .into_any_element()
                                            }),
                                    ),
                            ),
                    ),
            )
            .when(self.show_language_menu, |root| {
                root.child(
                    div()
                        .absolute()
                        .top(px(88.))
                        .right(px(28.))
                        .w(px(260.))
                        .rounded_xl()
                        .border_1()
                        .border_color(rgb(BORDER_SOFT))
                        .bg(rgb(SURFACE_ELEVATED))
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(self.i18n.tr("selector.change_language")),
                        )
                        .children(ALL_LANGUAGES.iter().copied().map(|language| {
                            let active = self.i18n.language() == language;
                            language_row(
                                self.i18n.language_label(language),
                                language.as_code(),
                                active,
                            )
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _, window, cx| {
                                    view.cycle_language(language, window, cx);
                                }),
                            )
                        })),
                )
            })
            .when(self.show_form, |root| {
                root.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .bg(rgb(0x060b10))
                        .opacity(0.95)
                        .child(
                            div()
                                .mx_auto()
                                .mt(px(48.))
                                .w(px(560.))
                                .rounded_xl()
                                .border_1()
                                .border_color(rgb(BORDER_SOFT))
                                .bg(rgb(SURFACE_ELEVATED))
                                .p_4()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child(if self.form.id.is_some() {
                                            self.i18n.tr("form.edit")
                                        } else {
                                            self.i18n.tr("form.new")
                                        }),
                                )
                                .child(field_block(
                                    self.i18n.tr("form.database_kind"),
                                    render_kind_selector(self.form.database_kind, cx),
                                ))
                                .child(field_block(
                                    self.i18n.tr("form.name"),
                                    self.form.name.clone(),
                                ))
                                .child(field_block(
                                    self.i18n.tr("form.host"),
                                    self.form.host.clone(),
                                ))
                                .child(field_block(
                                    self.i18n.tr("form.port"),
                                    self.form.port.clone(),
                                ))
                                .child(field_block(
                                    self.i18n.tr("form.database"),
                                    self.form.database.clone(),
                                ))
                                .child(field_block(
                                    self.i18n.tr("form.username"),
                                    self.form.username.clone(),
                                ))
                                .child(field_block(
                                    self.i18n.tr("form.password"),
                                    self.form.password.clone(),
                                ))
                                .child(field_block(
                                    self.i18n.tr("form.group"),
                                    self.form.group.clone(),
                                ))
                                .child(field_block(
                                    self.i18n.tr("form.color"),
                                    self.form.color_tag.clone(),
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .justify_end()
                                        .gap_2()
                                        .child(
                                            subtle_button(
                                                "×",
                                                self.i18n.tr("actions.cancel"),
                                            )
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::on_cancel_form),
                                            ),
                                        )
                                        .child(
                                            subtle_button(
                                                "◌",
                                                self.i18n.tr("actions.test"),
                                            )
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::on_test_connection),
                                            ),
                                        )
                                        .child(
                                            accent_button(
                                                "✓",
                                                self.i18n.tr("actions.save"),
                                            )
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(Self::on_save_connection),
                                            ),
                                        ),
                                ),
                        ),
                )
            })
    }
}

fn optional_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn group_connections(
    connections: Vec<ConnectionSummary>,
    fallback_group: &str,
) -> BTreeMap<String, Vec<ConnectionSummary>> {
    let mut groups = BTreeMap::new();
    for connection in connections {
        let group = connection
            .group
            .clone()
            .unwrap_or_else(|| fallback_group.to_string());
        groups.entry(group).or_insert_with(Vec::new).push(connection);
    }
    groups
}

fn connection_identity(connection: &ConnectionSummary) -> String {
    let host = connection.host.clone().unwrap_or_else(|| "local".to_string());
    let database = connection.database.clone().unwrap_or_default();
    if database.is_empty() {
        host
    } else {
        format!("{host} · {database}")
    }
}

fn brand_mark() -> impl IntoElement {
    div()
        .size_12()
        .rounded_xl()
        .bg(rgb(ACCENT_SOFT))
        .border_1()
        .border_color(rgb(BORDER_SOFT))
        .justify_center()
        .items_center()
        .flex()
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(rgb(ACCENT))
        .child("DZ")
}

fn logo_badge() -> impl IntoElement {
    div()
        .size_11()
        .rounded_xl()
        .bg(rgb(ACCENT_SOFT))
        .justify_center()
        .items_center()
        .flex()
        .text_color(rgb(ACCENT))
        .font_weight(gpui::FontWeight::BOLD)
        .child("◈")
}

fn rail_button(icon: &'static str, _label: impl Into<SharedString>) -> gpui::Div {
    div()
        .w(px(48.))
        .h(px(48.))
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE_ALT))
        .cursor_pointer()
        .justify_center()
        .items_center()
        .flex()
        .text_color(white())
        .child(icon)
}

fn meta_pill(
    icon: &'static str,
    text: &str,
    background: u32,
    foreground: u32,
) -> impl IntoElement {
    div()
        .rounded_full()
        .bg(rgb(background))
        .border_1()
        .border_color(rgb(BORDER))
        .px_3()
        .py_1()
        .flex()
        .items_center()
        .gap_2()
        .child(div().text_xs().text_color(rgb(foreground)).child(icon))
        .child(div().text_sm().text_color(rgb(foreground)).child(text.to_string()))
}

fn toolbar_button(icon: &'static str, text: impl Into<SharedString>, bg: u32, fg: u32) -> gpui::Div {
    let text: SharedString = text.into();
    div()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(bg))
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_2()
        .cursor_pointer()
        .text_color(rgb(fg))
        .child(div().child(icon))
        .child(div().text_sm().child(text))
}

fn subtle_button(icon: &'static str, text: impl Into<SharedString>) -> gpui::Div {
    toolbar_button(icon, text, SURFACE_ALT, 0xffffff)
}

fn accent_button(icon: &'static str, text: impl Into<SharedString>) -> gpui::Div {
    toolbar_button(icon, text, ACCENT, 0x06121d)
}

fn danger_button(icon: &'static str, text: impl Into<SharedString>) -> gpui::Div {
    toolbar_button(icon, text, 0x3a1e24, 0xffffff)
}

fn group_label(text: impl Into<SharedString>) -> impl IntoElement {
    let text: SharedString = text.into();
    div()
        .pt_2()
        .text_xs()
        .text_color(rgb(TEXT_SOFT))
        .child(text)
}

fn empty_notice(text: impl Into<SharedString>) -> impl IntoElement {
    let text: SharedString = text.into();
    div()
        .rounded_lg()
        .bg(rgb(SURFACE_ALT))
        .border_1()
        .border_color(rgb(BORDER))
        .p_3()
        .text_sm()
        .text_color(rgb(TEXT_MUTED))
        .child(text)
}

fn sidebar_empty_state(
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
    supported_label: impl Into<SharedString>,
) -> impl IntoElement {
    let title: SharedString = title.into();
    let hint: SharedString = hint.into();
    let supported_label: SharedString = supported_label.into();
    div()
        .rounded_xl()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE_ALT))
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size_9()
                        .rounded_lg()
                        .bg(rgb(ACCENT_SOFT))
                        .justify_center()
                        .items_center()
                        .flex()
                        .text_color(rgb(ACCENT))
                        .child("◈"),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(title),
                ),
        )
        .child(div().text_sm().text_color(rgb(TEXT_MUTED)).child(hint))
        .child(
            div()
                .text_xs()
                .text_color(rgb(TEXT_SOFT))
                .child(supported_label),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .flex_wrap()
                .child(support_chip("Pg", db_kind_color(DatabaseKind::PostgreSql)))
                .child(support_chip("My", db_kind_color(DatabaseKind::MySql)))
                .child(support_chip("Ma", db_kind_color(DatabaseKind::MariaDb)))
                .child(support_chip("Sq", db_kind_color(DatabaseKind::Sqlite)))
                .child(support_chip("Rd", db_kind_color(DatabaseKind::Redis))),
        )
}

fn sidebar_quickstart(
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
) -> gpui::Div {
    let title: SharedString = title.into();
    let hint: SharedString = hint.into();
    div()
        .rounded_xl()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE_ALT))
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::BOLD)
                .child(title),
        )
        .child(div().text_sm().text_color(rgb(TEXT_MUTED)).child(hint))
}

fn quick_start_button(
    badge: &'static str,
    text: impl Into<SharedString>,
    badge_color: u32,
) -> gpui::Div {
    let text: SharedString = text.into();
    div()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_3()
        .cursor_pointer()
        .child(
            div()
                .size_8()
                .rounded_lg()
                .bg(rgb(badge_color))
                .justify_center()
                .items_center()
                .flex()
                .text_color(rgb(0x081018))
                .font_weight(gpui::FontWeight::BOLD)
                .child(badge),
        )
        .child(div().text_sm().child(text))
}

fn support_chip(label: &'static str, color: u32) -> impl IntoElement {
    div()
        .rounded_full()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .px_2()
        .py_1()
        .flex()
        .items_center()
        .gap_2()
        .child(div().text_color(rgb(color)).child("●"))
        .child(div().text_sm().child(label))
}

fn workspace_panel(title: impl Into<SharedString>) -> gpui::Div {
    let title: SharedString = title.into();
    div()
        .flex_1()
        .rounded_xl()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::BOLD)
                .child(title),
        )
}

fn workspace_panel_with_action(title: impl Into<SharedString>, action: impl Into<SharedString>) -> gpui::Div {
    let title: SharedString = title.into();
    let action: SharedString = action.into();
    div()
        .flex_1()
        .rounded_xl()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(title),
                )
                .child(
                    div()
                        .rounded_full()
                        .bg(rgb(SURFACE_ALT))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(rgb(TEXT_MUTED))
                        .child(action),
                ),
        )
}

fn list_row(icon: &'static str, title: &str, detail: impl Into<SharedString>, selected: bool) -> gpui::Div {
    let detail: SharedString = detail.into();
    div()
        .rounded_lg()
        .border_1()
        .border_color(rgb(if selected { ACCENT } else { BORDER }))
        .bg(rgb(if selected { ACCENT_SOFT } else { SURFACE_ALT }))
        .px_3()
        .py_2()
        .flex()
        .justify_between()
        .items_center()
        .cursor_pointer()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_color(rgb(if selected { ACCENT } else { TEXT_MUTED })).child(icon))
                .child(div().text_sm().child(title.to_string())),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(TEXT_SOFT))
                .child(detail),
        )
}

fn connection_card(connection: &ConnectionSummary, selected: bool) -> gpui::Div {
    let accent = db_kind_color(connection.database_kind);
    div()
        .rounded_xl()
        .border_1()
        .border_color(rgb(if selected { ACCENT } else { BORDER }))
        .bg(rgb(if selected { ACCENT_SOFT } else { SURFACE_ALT }))
        .p_3()
        .flex()
        .gap_3()
        .cursor_pointer()
        .child(div().w(px(4.)).rounded_full().bg(rgb(accent)))
        .child(
            div()
                .size_10()
                .rounded_lg()
                .bg(rgb(accent))
                .justify_center()
                .items_center()
                .flex()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(0x081018))
                .child(db_kind_name(connection.database_kind)),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(connection.name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(TEXT_SOFT))
                                .child(database_kind_code(connection.database_kind)),
                        ),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(TEXT_MUTED))
                        .child(connection.host.clone().unwrap_or_else(|| "local".to_string())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(TEXT_SOFT))
                        .child(connection.database.clone().unwrap_or_default()),
                ),
        )
}

fn terminal_panel(title: impl Into<SharedString>, helper: impl Into<SharedString>, identity: String) -> gpui::Div {
    let title: SharedString = title.into();
    let helper: SharedString = helper.into();
    div()
        .h(px(292.))
        .rounded_xl()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(TEXT_SOFT))
                        .child(identity),
                ),
        )
        .child(
            div()
                .rounded_xl()
                .border_1()
                .border_color(rgb(TERMINAL_EDGE))
                .bg(rgb(TERMINAL_BG))
                .p_3()
                .flex()
                .flex_col()
                .gap_3()
                .flex_1()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().text_color(rgb(DANGER)).child("●"))
                        .child(div().text_color(rgb(WARNING)).child("●"))
                        .child(div().text_color(rgb(SUCCESS)).child("●")),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(TEXT_MUTED))
                        .child(helper),
                ),
        )
}

fn language_row(label: impl Into<SharedString>, code: &str, active: bool) -> gpui::Div {
    let label: SharedString = label.into();
    div()
        .rounded_lg()
        .border_1()
        .border_color(rgb(if active { ACCENT } else { BORDER }))
        .bg(rgb(if active { ACCENT_SOFT } else { SURFACE_ALT }))
        .px_3()
        .py_2()
        .flex()
        .justify_between()
        .items_center()
        .cursor_pointer()
        .child(div().text_sm().child(label))
        .child(
            div()
                .text_xs()
                .text_color(rgb(if active { ACCENT } else { TEXT_SOFT }))
                .child(code.to_string()),
        )
}

fn field_block(label: impl Into<SharedString>, child: impl IntoElement) -> impl IntoElement {
    let label: SharedString = label.into();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_sm().text_color(rgb(TEXT_MUTED)).child(label))
        .child(child)
}

fn render_kind_selector(kind: DatabaseKind, cx: &mut Context<RootView>) -> impl IntoElement {
    let kinds = [
        DatabaseKind::PostgreSql,
        DatabaseKind::MySql,
        DatabaseKind::MariaDb,
        DatabaseKind::Sqlite,
        DatabaseKind::Redis,
    ];

    div().flex().gap_2().flex_wrap().children(kinds.into_iter().map(|candidate| {
        let active = candidate == kind;
        language_row(db_kind_name(candidate), database_kind_code(candidate), active).on_mouse_up(
            MouseButton::Left,
            cx.listener(move |view, _, _, cx| {
                view.form.database_kind = candidate;
                cx.notify();
            }),
        )
    }))
}

fn render_preview(
    preview: Option<&TablePreview>,
    sample_label: impl Into<SharedString>,
    empty: impl Into<SharedString>,
    column_label: &str,
    row_label: &str,
) -> gpui::AnyElement {
    let sample_label: SharedString = sample_label.into();
    let empty: SharedString = empty.into();
    match preview {
        Some(preview) => div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .flex_wrap()
                    .child(meta_pill(
                        "▦",
                        &format!("{} {}", preview.columns.len(), column_label),
                        SURFACE_ALT,
                        TEXT_MUTED,
                    ))
                    .child(meta_pill(
                        "≡",
                        &format!("{} {}", preview.rows.len(), row_label),
                        SURFACE_ALT,
                        TEXT_MUTED,
                    )),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(TEXT_MUTED))
                    .child(sample_label),
            )
            .child(
                div()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE_ALT))
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(ACCENT))
                    .child(preview.columns.join("  |  ")),
            )
            .children(preview.rows.iter().take(12).enumerate().map(|(index, row)| {
                div()
                    .rounded_lg()
                    .bg(rgb(SURFACE_ALT))
                    .px_3()
                    .py_2()
                    .text_sm()
                    .child(format!("{:02}  {}", index + 1, row.join("  |  ")))
            }))
            .into_any_element(),
        None => empty_notice(empty).into_any_element(),
    }
}

fn render_query_terminal(
    result: Option<&QuerySnapshot>,
    empty: impl Into<SharedString>,
    row_label: &str,
    column_label: &str,
) -> gpui::AnyElement {
    let empty: SharedString = empty.into();
    match result {
        Some(result) => {
            let affected = result
                .rows_affected
                .map(|value| format!(" · {value} affected"))
                .unwrap_or_default();
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(TERMINAL_GREEN))
                        .child(format!(
                            "{} {} · {} {} · {} ms{}",
                            result.rows.len(),
                            row_label,
                            result.columns.len(),
                            column_label,
                            result.execution_time_ms,
                            affected
                        )),
                )
                .child(
                    div()
                        .rounded_lg()
                        .bg(rgb(0x0d1824))
                        .px_3()
                        .py_2()
                        .text_sm()
                        .text_color(rgb(TERMINAL_AMBER))
                        .child(result.columns.join(" | ")),
                )
                .when(result.rows.is_empty(), |panel| panel.child(empty_notice(empty.clone())))
                .children(result.rows.iter().take(12).enumerate().map(|(index, row)| {
                    div()
                        .rounded_lg()
                        .bg(rgb(0x0c1620))
                        .px_3()
                        .py_2()
                        .text_sm()
                        .text_color(white())
                        .child(format!("{:02} | {}", index + 1, row.join(" | ")))
                }))
                .into_any_element()
        }
        None => empty_notice(empty).into_any_element(),
    }
}

fn db_kind_name(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::PostgreSql => "Pg",
        DatabaseKind::MySql => "My",
        DatabaseKind::MariaDb => "Ma",
        DatabaseKind::Sqlite => "Sq",
        DatabaseKind::Redis => "Rd",
    }
}

fn db_kind_color(kind: DatabaseKind) -> u32 {
    match kind {
        DatabaseKind::PostgreSql => 0x72adff,
        DatabaseKind::MySql => 0xf5b85c,
        DatabaseKind::MariaDb => 0x8dc8ff,
        DatabaseKind::Sqlite => 0x69d6b0,
        DatabaseKind::Redis => 0xff7d7d,
    }
}

fn database_kind_code(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::PostgreSql => "postgres",
        DatabaseKind::MySql => "mysql",
        DatabaseKind::MariaDb => "mariadb",
        DatabaseKind::Sqlite => "sqlite",
        DatabaseKind::Redis => "redis",
    }
}

pub fn run() -> Result<()> {
    let gateway = Arc::new(BackendGateway::new()?);

    Application::new().run(move |cx: &mut App| {
        bind_input_keys(cx);

        let query_input = TextInput::new("", "", false, cx);
        let connection_filter = TextInput::new("", "", false, cx);
        let form = ConnectionForm::new(cx);
        let gateway = gateway.clone();

        let window = cx
            .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1500.), px(980.)),
                    cx,
                ))),
                ..Default::default()
            },
            move |_window, cx| {
                let root = cx.new(|_| RootView {
                    gateway: gateway.clone(),
                    i18n: I18n::new(UiLanguage::English),
                    theme: ThemeMode::Graphite,
                    query_input: query_input.clone(),
                    connection_filter: connection_filter.clone(),
                    form,
                    connections: Vec::new(),
                    selected_connection_id: None,
                    session: None,
                    databases: Vec::new(),
                    selected_database: None,
                    tables: Vec::new(),
                    selected_table: None,
                    table_preview: None,
                    query_result: None,
                    show_form: false,
                    show_language_menu: false,
                    busy_label: None,
                    status: "Ready".into(),
                    error: None,
                });

                root.update(cx, |view, cx| {
                    view.sync_locale_inputs(cx);
                    view.bootstrap(cx);
                });
                root
            },
        )
            .expect("window must open");

        window
            .update(cx, |_, window, _| {
                window.activate_window();
            })
            .ok();

    });

    Ok(())
}
