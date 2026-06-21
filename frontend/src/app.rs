use std::sync::Arc;

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
use crate::input::{bind_input_keys, TextInput};

const CANVAS: u32 = 0x081019;
const PANEL: u32 = 0x101820;
const PANEL_SOFT: u32 = 0x132434;
const PANEL_EDGE: u32 = 0x203341;
const ACCENT: u32 = 0x19c37d;
const MUTED: u32 = 0x7d94a5;
const DANGER: u32 = 0xb84f4f;

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
            color_tag: TextInput::new("", "emerald", false, cx),
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
    busy_label: Option<SharedString>,
    status: SharedString,
    error: Option<SharedString>,
}

impl RootView {
    fn on_reload(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.bootstrap(cx);
    }

    fn on_new_connection(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.form.clear(cx);
        self.show_form = true;
        self.error = None;
        cx.notify();
    }

    fn on_cancel_form(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.show_form = false;
        cx.notify();
    }

    fn on_save_connection(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let draft = self.form.to_draft(cx);
        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.saving").into());
        self.error = None;
        cx.notify();

        let task = cx.background_executor().spawn(async move { gateway.save_connection(draft) });
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
                    Err(error) => {
                        view.set_error(error.to_string(), cx);
                    }
            });
        })
        .detach();
    }

    fn on_test_connection(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let draft = self.form.to_draft(cx);
        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.testing").into());
        self.error = None;
        cx.notify();

        let task = cx.background_executor().spawn(async move { gateway.test_connection(draft) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                    Ok(server) => {
                        view.status =
                            format!("{}: {} {}", view.i18n.tr("status.connected"), server.server_type, server.server_version).into();
                        view.busy_label = None;
                        cx.notify();
                    }
                    Err(error) => view.set_error(error.to_string(), cx),
            });
        })
        .detach();
    }

    fn on_edit_selected(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_connection_id.clone() else {
            return;
        };

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.loading").into());
        cx.notify();

        let task = cx.background_executor().spawn(async move { gateway.load_connection(id) });
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

    fn on_delete_selected(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_connection_id.clone() else {
            return;
        };

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.loading").into());
        cx.notify();

        let task = cx.background_executor().spawn(async move { gateway.delete_connection(id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                    Ok(()) => {
                        if let Some(selected) = &view.selected_connection_id {
                            view.connections.retain(|connection| &connection.id != selected);
                        }
                        view.selected_connection_id = view.connections.first().map(|connection| connection.id.clone());
                        view.session = None;
                        view.databases.clear();
                        view.tables.clear();
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

    fn on_connect_selected(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_connection_id.clone() else {
            return;
        };

        let gateway = self.gateway.clone();
        self.busy_label = Some(self.i18n.tr("status.connecting").into());
        cx.notify();

        let task = cx.background_executor().spawn(async move { gateway.connect(id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| match result {
                    Ok(session) => {
                        view.status =
                            format!("{}: {} {}", view.i18n.tr("status.connected"), session.server_type, session.server_version).into();
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

    fn on_disconnect(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
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
        let (Some(session), Some(database)) = (self.session.clone(), self.selected_database.clone()) else {
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

    fn on_preview_table(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    fn on_run_query(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    fn cycle_language(&mut self, language: UiLanguage, _: &mut Window, cx: &mut Context<Self>) {
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

    fn select_database(&mut self, database: String, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_database = Some(database);
        self.tables.clear();
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
        if let Some(existing) = self.connections.iter_mut().find(|item| item.id == connection.id) {
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
        self.selected_connection_id = self.connections.first().map(|connection| connection.id.clone());
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
            .and_then(|id| self.connections.iter().find(|connection| &connection.id == id));

        div()
            .relative()
            .size_full()
            .bg(rgb(CANVAS))
            .text_color(white())
            .child(
                div()
                    .flex()
                    .size_full()
                    .child(
                        div()
                            .w(px(320.))
                            .bg(rgb(PANEL))
                            .border_r_1()
                            .border_color(rgb(PANEL_EDGE))
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_xl().font_weight(gpui::FontWeight::BOLD).child(self.i18n.tr("app.title")))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(MUTED))
                                            .child(self.i18n.tr("app.subtitle")),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(action_button(self.i18n.tr("action.reload")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_reload)))
                                    .child(action_button(self.i18n.tr("action.new")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_new_connection))),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(MUTED))
                                    .child(self.i18n.tr("sidebar.connections")),
                            )
                            .children(self.connections.iter().map(|connection| {
                                let selected = self
                                    .selected_connection_id
                                    .as_ref()
                                    .map(|id| id == &connection.id)
                                    .unwrap_or(false);
                                let id = connection.id.clone();
                                let label = db_badge_label(connection.database_kind);
                                connection_card(connection, selected, label)
                                    .on_mouse_up(MouseButton::Left, cx.listener(move |view, _, _, cx| {
                                        view.select_connection(id.clone(), cx);
                                    }))
                            }))
                            .when(self.connections.is_empty(), |panel| {
                                panel.child(
                                    div()
                                        .p_3()
                                        .rounded_md()
                                        .bg(rgb(PANEL_SOFT))
                                        .text_sm()
                                        .text_color(rgb(MUTED))
                                        .child(self.i18n.tr("sidebar.no_connections")),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .pt_2()
                                    .children(ALL_LANGUAGES.iter().copied().map(|language| {
                                        let active = self.i18n.language() == language;
                                        language_chip(self.i18n.language_label(language), active)
                                            .on_mouse_up(MouseButton::Left, cx.listener(move |view, _, window, cx| {
                                                view.cycle_language(language, window, cx);
                                            }))
                                    })),
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
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xl()
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .child(selected_connection.map(|connection| connection.name.clone()).unwrap_or_else(|| self.i18n.tr("app.title").to_string())),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(rgb(MUTED))
                                                    .child(selected_connection
                                                        .and_then(|connection| connection.host.clone())
                                                        .unwrap_or_else(|| self.i18n.tr("content.select_connection").to_string())),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(action_button(self.i18n.tr("action.edit")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_edit_selected)))
                                            .child(danger_button(self.i18n.tr("action.delete")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_delete_selected)))
                                            .child(match self.session {
                                                Some(_) => action_button(self.i18n.tr("action.disconnect")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_disconnect)).into_any_element(),
                                                None => accent_button(self.i18n.tr("action.connect")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_connect_selected)).into_any_element(),
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(220.))
                                            .bg(rgb(PANEL))
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(rgb(PANEL_EDGE))
                                            .p_3()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(section_title(self.i18n.tr("content.databases")))
                                            .children(self.databases.iter().map(|database| {
                                                let selected = self.selected_database.as_ref() == Some(database);
                                                let db = database.clone();
                                                list_row(database, selected).on_mouse_up(MouseButton::Left, cx.listener(move |view, _, window, cx| {
                                                    view.select_database(db.clone(), window, cx);
                                                }))
                                            }))
                                            .when(self.databases.is_empty(), |panel| {
                                                panel.child(empty_notice(if self.session.is_some() {
                                                    self.i18n.tr("status.loading")
                                                } else {
                                                    self.i18n.tr("content.not_connected")
                                                }))
                                            }),
                                    )
                                    .child(
                                        div()
                                            .w(px(260.))
                                            .bg(rgb(PANEL))
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(rgb(PANEL_EDGE))
                                            .p_3()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .flex()
                                                    .justify_between()
                                                    .items_center()
                                                    .child(section_title(self.i18n.tr("content.tables")))
                                                    .child(action_button(self.i18n.tr("action.open_preview")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_preview_table))),
                                            )
                                            .children(self.tables.iter().map(|table| {
                                                let selected = self.selected_table.as_ref() == Some(&table.name);
                                                let name = table.name.clone();
                                                list_row(&table.name, selected).on_mouse_up(MouseButton::Left, cx.listener(move |view, _, _, cx| {
                                                    view.select_table(name.clone(), cx);
                                                }))
                                            }))
                                            .when(self.tables.is_empty(), |panel| panel.child(empty_notice(self.i18n.tr("preview.empty")))),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_col()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .bg(rgb(PANEL))
                                                    .rounded_lg()
                                                    .border_1()
                                                    .border_color(rgb(PANEL_EDGE))
                                                    .p_3()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_2()
                                                    .child(section_title(self.i18n.tr("content.preview")))
                                                    .child(render_preview(self.table_preview.as_ref(), self.i18n.tr("preview.empty"))),
                                            )
                                            .child(
                                                div()
                                                    .bg(rgb(PANEL))
                                                    .rounded_lg()
                                                    .border_1()
                                                    .border_color(rgb(PANEL_EDGE))
                                                    .p_3()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .justify_between()
                                                            .items_center()
                                                            .child(section_title(self.i18n.tr("content.query")))
                                                            .child(accent_button(self.i18n.tr("action.run_query")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_run_query))),
                                                    )
                                                    .child(self.query_input.clone())
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(rgb(MUTED))
                                                            .child(self.i18n.tr("query.placeholder")),
                                                    )
                                                    .child(section_title(self.i18n.tr("content.results")))
                                                    .child(render_query(self.query_result.as_ref(), self.i18n.tr("query.empty"))),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .h(px(40.))
                                    .rounded_lg()
                                    .bg(rgb(PANEL))
                                    .border_1()
                                    .border_color(rgb(PANEL_EDGE))
                                    .px_3()
                                    .items_center()
                                    .flex()
                                    .justify_between()
                                    .child(div().text_sm().text_color(rgb(MUTED)).child(status_label))
                                    .child(
                                        self.error
                                            .as_ref()
                                            .map(|error| div().text_sm().text_color(rgb(DANGER)).child(error.clone()).into_any_element())
                                            .unwrap_or_else(|| div().text_sm().text_color(rgb(ACCENT)).child(self.i18n.tr("status.ready")).into_any_element()),
                                    ),
                            ),
                    ),
            )
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
                                .mt(px(56.))
                                .w(px(520.))
                                .bg(rgb(PANEL))
                                .rounded_xl()
                                .border_1()
                                .border_color(rgb(PANEL_EDGE))
                                .p_4()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(section_title(if self.form.id.is_some() {
                                    self.i18n.tr("form.edit")
                                } else {
                                    self.i18n.tr("form.new")
                                }))
                                .child(field_block(self.i18n.tr("form.database_kind"), render_kind_selector(self.form.database_kind, cx)))
                                .child(field_block(self.i18n.tr("form.name"), self.form.name.clone()))
                                .child(field_block(self.i18n.tr("form.host"), self.form.host.clone()))
                                .child(field_block(self.i18n.tr("form.port"), self.form.port.clone()))
                                .child(field_block(self.i18n.tr("form.database"), self.form.database.clone()))
                                .child(field_block(self.i18n.tr("form.username"), self.form.username.clone()))
                                .child(field_block(self.i18n.tr("form.password"), self.form.password.clone()))
                                .child(field_block(self.i18n.tr("form.group"), self.form.group.clone()))
                                .child(field_block(self.i18n.tr("form.color"), self.form.color_tag.clone()))
                                .child(
                                    div()
                                        .flex()
                                        .justify_end()
                                        .gap_2()
                                        .child(action_button(self.i18n.tr("action.cancel")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_cancel_form)))
                                        .child(action_button(self.i18n.tr("action.test")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_test_connection)))
                                        .child(accent_button(self.i18n.tr("action.save")).on_mouse_up(MouseButton::Left, cx.listener(Self::on_save_connection))),
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

fn section_title(text: impl Into<SharedString>) -> impl IntoElement {
    let text: SharedString = text.into();
    div()
        .text_sm()
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(white())
        .child(text)
}

fn empty_notice(text: impl Into<SharedString>) -> impl IntoElement {
    let text: SharedString = text.into();
    div()
        .rounded_md()
        .bg(rgb(PANEL_SOFT))
        .p_3()
        .text_sm()
        .text_color(rgb(MUTED))
        .child(text)
}

fn action_button(text: impl Into<SharedString>) -> gpui::Div {
    let text: SharedString = text.into();
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(PANEL_SOFT))
        .border_1()
        .border_color(rgb(PANEL_EDGE))
        .text_sm()
        .cursor_pointer()
        .child(text)
}

fn accent_button(text: impl Into<SharedString>) -> gpui::Div {
    let text: SharedString = text.into();
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(ACCENT))
        .text_color(rgb(0x06120d))
        .text_sm()
        .cursor_pointer()
        .child(text)
}

fn danger_button(text: impl Into<SharedString>) -> gpui::Div {
    let text: SharedString = text.into();
    div()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(0x3d1616))
        .text_sm()
        .cursor_pointer()
        .child(text)
}

fn language_chip(text: impl Into<SharedString>, active: bool) -> gpui::Div {
    let text: SharedString = text.into();
    div()
        .px_2()
        .py_1()
        .rounded_full()
        .bg(if active { rgb(ACCENT) } else { rgb(PANEL_SOFT) })
        .text_color(if active { rgb(0x06120d) } else { rgb(0xffffff) })
        .text_sm()
        .cursor_pointer()
        .child(text)
}

fn list_row(text: impl Into<SharedString>, selected: bool) -> gpui::Div {
    let text: SharedString = text.into();
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(PANEL_EDGE))
        .bg(if selected { rgb(0x193246) } else { rgb(PANEL_SOFT) })
        .px_3()
        .py_2()
        .text_sm()
        .cursor_pointer()
        .child(text)
}

fn connection_card(connection: &ConnectionSummary, selected: bool, badge: &'static str) -> gpui::Div {
    div()
        .rounded_lg()
        .border_1()
        .border_color(rgb(PANEL_EDGE))
        .bg(if selected { rgb(0x183042) } else { rgb(PANEL_SOFT) })
        .p_3()
        .flex()
        .gap_3()
        .cursor_pointer()
        .child(
            div()
                .size_10()
                .rounded_lg()
                .bg(rgb(ACCENT))
                .text_color(rgb(0x06120d))
                .justify_center()
                .items_center()
                .flex()
                .font_weight(gpui::FontWeight::BOLD)
                .child(badge),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().child(connection.name.clone()))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(MUTED))
                        .child(connection.host.clone().unwrap_or_else(|| "local".to_string())),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(MUTED))
                        .child(connection.database.clone().unwrap_or_default()),
                ),
        )
}

fn field_block(label: impl Into<SharedString>, child: impl IntoElement) -> impl IntoElement {
    let label: SharedString = label.into();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_sm().text_color(rgb(MUTED)).child(label))
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

    div().flex().gap_2().children(kinds.into_iter().map(|candidate| {
        let active = candidate == kind;
        language_chip(db_kind_name(candidate), active).on_mouse_up(MouseButton::Left, cx.listener(move |view, _, _, cx| {
            view.form.database_kind = candidate;
            cx.notify();
        }))
    }))
}

fn render_preview(preview: Option<&TablePreview>, empty: impl Into<SharedString>) -> gpui::AnyElement {
    let empty: SharedString = empty.into();
    match preview {
        Some(preview) => div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(format!("{} columns, {} rows", preview.columns.len(), preview.rows.len())),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x9ed7bb))
                    .child(preview.columns.join(" | ")),
            )
            .children(preview.rows.iter().take(10).map(|row| {
                div().text_sm().text_color(white()).child(row.join(" | "))
            }))
            .into_any_element(),
        None => empty_notice(empty).into_any_element(),
    }
}

fn render_query(result: Option<&QuerySnapshot>, empty: impl Into<SharedString>) -> gpui::AnyElement {
    let empty: SharedString = empty.into();
    match result {
        Some(result) => div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(format!(
                        "{} columns, {} rows, {} ms",
                        result.columns.len(),
                        result.rows.len(),
                        result.execution_time_ms
                    )),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x9ed7bb))
                    .child(result.columns.join(" | ")),
            )
            .children(result.rows.iter().take(10).map(|row| {
                div().text_sm().child(row.join(" | "))
            }))
            .into_any_element(),
        None => empty_notice(empty).into_any_element(),
    }
}

fn db_badge_label(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::PostgreSql => "PG",
        DatabaseKind::MySql => "MY",
        DatabaseKind::MariaDb => "MA",
        DatabaseKind::Sqlite => "SQ",
        DatabaseKind::Redis => "RD",
    }
}

fn db_kind_name(kind: DatabaseKind) -> &'static str {
    match kind {
        DatabaseKind::PostgreSql => "PostgreSQL",
        DatabaseKind::MySql => "MySQL",
        DatabaseKind::MariaDb => "MariaDB",
        DatabaseKind::Sqlite => "SQLite",
        DatabaseKind::Redis => "Redis",
    }
}

pub fn run() -> Result<()> {
    let gateway = Arc::new(BackendGateway::new()?);

    Application::new().run(move |cx: &mut App| {
        bind_input_keys(cx);

        let query_input = TextInput::new("", "SELECT * FROM users LIMIT 100", false, cx);
        let form = ConnectionForm::new(cx);
        let gateway = gateway.clone();

        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1460.), px(920.)),
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
                        busy_label: Some("Loading".into()),
                        status: "Ready".into(),
                        error: None,
                    });

                    root.update(cx, |view, cx| {
                        view.bootstrap(cx);
                    });

                    root
                },
            )
            .expect("failed to open main window");

        window
            .update(cx, |_, _, cx| {
                cx.activate(true);
            })
            .ok();
    });

    Ok(())
}
