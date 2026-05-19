use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;

use crate::commands::tree::progress_glyph;
use crate::graph;
use crate::model::frontmatter::{Progress, Status, TypeSpecificFields};
use crate::model::registry::SpecRegistry;

pub fn run(specs_dir: &Path) -> Result<()> {
    let registry = SpecRegistry::load(specs_dir)?;
    let mut app = App::new(registry);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = event_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            // Filter input mode swallows most keys
            if app.filter_mode {
                match key.code {
                    KeyCode::Esc => {
                        app.filter_mode = false;
                        app.filter.clear();
                        app.rebuild_visible();
                    }
                    KeyCode::Enter => {
                        app.filter_mode = false;
                    }
                    KeyCode::Backspace => {
                        app.filter.pop();
                        app.rebuild_visible();
                    }
                    KeyCode::Char(c) => {
                        app.filter.push(c);
                        app.rebuild_visible();
                    }
                    _ => {}
                }
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(());
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.half_page_down();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.half_page_up();
                }
                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.scroll_down();
                }
                KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.scroll_up();
                }
                KeyCode::Char('j') | KeyCode::Down => app.move_selection(1),
                KeyCode::Char('k') | KeyCode::Up => app.move_selection(-1),
                KeyCode::Char('g') => app.set_selection(0),
                KeyCode::Char('G') => app.set_selection(isize::MAX),
                KeyCode::PageDown => app.move_selection(10),
                KeyCode::PageUp => app.move_selection(-10),
                KeyCode::Char('h') | KeyCode::Left => app.collapse(),
                KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => app.expand_or_select(),
                KeyCode::Char(' ') => app.toggle(),
                KeyCode::Char('/') => {
                    app.filter_mode = true;
                    app.filter.clear();
                    app.rebuild_visible();
                }
                _ => {}
            }
        }
    }
}

#[derive(Clone, Copy)]
enum NodeKind {
    Namespace,
    Type,
    Spec,
}

struct Node {
    kind: NodeKind,
    label: String,
    /// Depth in tree (0 = namespace).
    depth: u16,
    /// Index into registry.documents — only meaningful for Spec nodes.
    doc_idx: Option<usize>,
    /// Group key (namespace) or (namespace, type) for non-spec nodes.
    namespace: String,
    entity_type: Option<&'static str>,
}

struct App {
    registry: SpecRegistry,
    /// All nodes in tree order (DFS).
    all_nodes: Vec<Node>,
    /// Currently-collapsed group keys.
    collapsed: std::collections::HashSet<String>,
    /// Indices into `all_nodes` that are currently visible (matches filter
    /// and parents are expanded).
    visible: Vec<usize>,
    list_state: ListState,
    filter: String,
    filter_mode: bool,
    /// Viewport height of the tree list (inner area, excluding borders).
    /// Updated each draw; used to size half-page jumps.
    viewport_height: usize,
}

impl App {
    fn new(registry: SpecRegistry) -> Self {
        // Build tree: namespace -> type -> [doc_idx]
        let mut grouped: BTreeMap<String, BTreeMap<&'static str, Vec<usize>>> = BTreeMap::new();
        for (idx, doc) in registry.documents.iter().enumerate() {
            let ns = doc.universal.id.namespace.clone();
            let ty = doc.universal.entity_type.prefix();
            grouped.entry(ns).or_default().entry(ty).or_default().push(idx);
        }

        let mut all_nodes = Vec::new();
        for (ns, types) in &grouped {
            all_nodes.push(Node {
                kind: NodeKind::Namespace,
                label: format!("{ns}/"),
                depth: 0,
                doc_idx: None,
                namespace: ns.clone(),
                entity_type: None,
            });
            for (ty, idxs) in types {
                all_nodes.push(Node {
                    kind: NodeKind::Type,
                    label: (*ty).to_string(),
                    depth: 1,
                    doc_idx: None,
                    namespace: ns.clone(),
                    entity_type: Some(*ty),
                });
                let mut idxs = idxs.clone();
                idxs.sort_by(|a, b| {
                    registry.documents[*a]
                        .universal
                        .id
                        .slug
                        .cmp(&registry.documents[*b].universal.id.slug)
                });
                for doc_idx in idxs {
                    let doc = &registry.documents[doc_idx];
                    all_nodes.push(Node {
                        kind: NodeKind::Spec,
                        label: doc.universal.id.slug.clone(),
                        depth: 2,
                        doc_idx: Some(doc_idx),
                        namespace: ns.clone(),
                        entity_type: Some(*ty),
                    });
                }
            }
        }

        let mut state = ListState::default();
        state.select(Some(0));

        let mut app = Self {
            registry,
            all_nodes,
            collapsed: Default::default(),
            visible: Vec::new(),
            list_state: state,
            filter: String::new(),
            filter_mode: false,
            viewport_height: 20,
        };
        app.rebuild_visible();
        app
    }

    fn group_key(node: &Node) -> Option<String> {
        match node.kind {
            NodeKind::Namespace => Some(node.namespace.clone()),
            NodeKind::Type => Some(format!(
                "{}::{}",
                node.namespace,
                node.entity_type.unwrap_or("")
            )),
            NodeKind::Spec => None,
        }
    }

    fn parent_key_of(node: &Node) -> Option<String> {
        match node.kind {
            NodeKind::Namespace => None,
            NodeKind::Type => Some(node.namespace.clone()),
            NodeKind::Spec => Some(format!(
                "{}::{}",
                node.namespace,
                node.entity_type.unwrap_or("")
            )),
        }
    }

    fn rebuild_visible(&mut self) {
        let filter_lc = self.filter.to_lowercase();
        let mut visible = Vec::new();
        // First pass: determine which spec nodes pass the filter
        let mut keep_spec: Vec<bool> = Vec::with_capacity(self.all_nodes.len());
        for node in &self.all_nodes {
            let pass = match node.kind {
                NodeKind::Spec => {
                    if filter_lc.is_empty() {
                        true
                    } else {
                        let doc = &self.registry.documents[node.doc_idx.unwrap()];
                        let id = doc.id_str().to_lowercase();
                        let summary = doc
                            .universal
                            .summary
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase();
                        id.contains(&filter_lc) || summary.contains(&filter_lc)
                    }
                }
                _ => true,
            };
            keep_spec.push(pass);
        }
        // Determine which Type nodes have any visible spec under them, and
        // which Namespace nodes have any visible Type under them.
        let mut type_has_visible: BTreeMap<String, bool> = BTreeMap::new();
        let mut ns_has_visible: BTreeMap<String, bool> = BTreeMap::new();
        for (i, node) in self.all_nodes.iter().enumerate() {
            if matches!(node.kind, NodeKind::Spec) && keep_spec[i] {
                let tk = format!("{}::{}", node.namespace, node.entity_type.unwrap_or(""));
                type_has_visible.insert(tk, true);
                ns_has_visible.insert(node.namespace.clone(), true);
            }
        }

        for (i, node) in self.all_nodes.iter().enumerate() {
            match node.kind {
                NodeKind::Namespace => {
                    if *ns_has_visible.get(&node.namespace).unwrap_or(&false) {
                        visible.push(i);
                    }
                }
                NodeKind::Type => {
                    let tk = format!("{}::{}", node.namespace, node.entity_type.unwrap_or(""));
                    if !type_has_visible.get(&tk).copied().unwrap_or(false) {
                        continue;
                    }
                    // Check parent namespace not collapsed
                    if self.collapsed.contains(&node.namespace) {
                        continue;
                    }
                    visible.push(i);
                }
                NodeKind::Spec => {
                    if !keep_spec[i] {
                        continue;
                    }
                    if self.collapsed.contains(&node.namespace) {
                        continue;
                    }
                    let tk = format!("{}::{}", node.namespace, node.entity_type.unwrap_or(""));
                    if self.collapsed.contains(&tk) {
                        continue;
                    }
                    visible.push(i);
                }
            }
        }

        self.visible = visible;
        if self.visible.is_empty() {
            self.list_state.select(None);
        } else {
            let sel = self.list_state.selected().unwrap_or(0).min(self.visible.len() - 1);
            self.list_state.select(Some(sel));
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let cur = self.list_state.selected().unwrap_or(0) as isize;
        let new = (cur + delta).clamp(0, self.visible.len() as isize - 1) as usize;
        self.list_state.select(Some(new));
    }

    fn set_selection(&mut self, idx: isize) {
        if self.visible.is_empty() {
            return;
        }
        let new = idx.clamp(0, self.visible.len() as isize - 1) as usize;
        self.list_state.select(Some(new));
    }

    /// Ctrl+d — move selection down by half a viewport.
    fn half_page_down(&mut self) {
        let step = (self.viewport_height / 2).max(1) as isize;
        self.move_selection(step);
    }

    /// Ctrl+u — move selection up by half a viewport.
    fn half_page_up(&mut self) {
        let step = (self.viewport_height / 2).max(1) as isize;
        self.move_selection(-step);
    }

    /// Ctrl+e — scroll viewport down one line. Cursor stays on its line
    /// unless that line scrolls off, in which case it follows.
    fn scroll_down(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let max_offset = self.visible.len().saturating_sub(1);
        let cur_offset = *self.list_state.offset_mut();
        if cur_offset < max_offset {
            *self.list_state.offset_mut() = cur_offset + 1;
        }
        // If selection scrolled off the top, push it down.
        if let Some(sel) = self.list_state.selected() {
            let new_offset = *self.list_state.offset_mut();
            if sel < new_offset {
                self.list_state.select(Some(new_offset));
            }
        }
    }

    /// Ctrl+y — scroll viewport up one line.
    fn scroll_up(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        let cur_offset = *self.list_state.offset_mut();
        if cur_offset == 0 {
            return;
        }
        *self.list_state.offset_mut() = cur_offset - 1;
        // If selection scrolled off the bottom, pull it up.
        if let Some(sel) = self.list_state.selected() {
            let new_offset = *self.list_state.offset_mut();
            let last_visible = new_offset + self.viewport_height.saturating_sub(1);
            if sel > last_visible {
                self.list_state.select(Some(last_visible));
            }
        }
    }

    fn current_node(&self) -> Option<&Node> {
        let sel = self.list_state.selected()?;
        let abs = *self.visible.get(sel)?;
        Some(&self.all_nodes[abs])
    }

    fn collapse(&mut self) {
        let action: Option<(String, NodeKind, String, Option<&'static str>)> =
            self.current_node().map(|node| {
                (
                    Self::group_key(node).unwrap_or_else(|| {
                        Self::parent_key_of(node).unwrap_or_default()
                    }),
                    node.kind,
                    node.namespace.clone(),
                    node.entity_type,
                )
            });
        if let Some((key, kind, parent_ns, parent_ty)) = action {
            self.collapsed.insert(key);
            if matches!(kind, NodeKind::Spec) {
                if let Some((vi, _)) =
                    self.visible.iter().enumerate().find(|(_, &abs)| {
                        let n = &self.all_nodes[abs];
                        matches!(n.kind, NodeKind::Type)
                            && n.namespace == parent_ns
                            && n.entity_type == parent_ty
                    })
                {
                    self.list_state.select(Some(vi));
                }
            }
            self.rebuild_visible();
        }
    }

    fn expand_or_select(&mut self) {
        if let Some(node) = self.current_node() {
            if let Some(key) = Self::group_key(node) {
                self.collapsed.remove(&key);
                self.rebuild_visible();
            }
        }
    }

    fn toggle(&mut self) {
        if let Some(node) = self.current_node() {
            if let Some(key) = Self::group_key(node) {
                if self.collapsed.contains(&key) {
                    self.collapsed.remove(&key);
                } else {
                    self.collapsed.insert(key);
                }
                self.rebuild_visible();
            }
        }
    }
}

fn draw(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[0]);

    draw_tree(f, app, body[0]);
    draw_detail(f, app, body[1]);
    draw_status(f, app, chunks[1]);
}

fn draw_tree(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    // area includes the border; the inner list region is height-2.
    app.viewport_height = (area.height as usize).saturating_sub(2).max(1);
    let items: Vec<ListItem> = app
        .visible
        .iter()
        .map(|&abs| {
            let node = &app.all_nodes[abs];
            let indent = "  ".repeat(node.depth as usize);
            match node.kind {
                NodeKind::Namespace => {
                    let marker = if app.collapsed.contains(&node.namespace) {
                        "▸"
                    } else {
                        "▾"
                    };
                    ListItem::new(Line::from(vec![
                        Span::raw(indent),
                        Span::raw(marker),
                        Span::raw(" "),
                        Span::styled(
                            node.label.clone(),
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ),
                    ]))
                }
                NodeKind::Type => {
                    let key = format!(
                        "{}::{}",
                        node.namespace,
                        node.entity_type.unwrap_or("")
                    );
                    let marker = if app.collapsed.contains(&key) { "▸" } else { "▾" };
                    ListItem::new(Line::from(vec![
                        Span::raw(indent),
                        Span::raw(marker),
                        Span::raw(" "),
                        Span::styled(node.label.clone(), type_style(&node.label)),
                    ]))
                }
                NodeKind::Spec => {
                    let doc = &app.registry.documents[node.doc_idx.unwrap()];
                    let status_color = status_color(doc.universal.status);
                    let mut spans = vec![
                        Span::raw(indent),
                        Span::raw("  "),
                        Span::raw(node.label.clone()),
                        Span::raw(" "),
                    ];
                    if let TypeSpecificFields::Task { progress, .. } = &doc.type_fields {
                        let (glyph, _) = progress_glyph(*progress);
                        spans.push(Span::styled(
                            format!("{glyph} "),
                            progress_style(*progress),
                        ));
                    }
                    spans.push(Span::styled(
                        format!("[{}]", doc.universal.status.as_str()),
                        Style::default().fg(status_color),
                    ));
                    ListItem::new(Line::from(spans))
                }
            }
        })
        .collect();

    let title = if app.filter_mode || !app.filter.is_empty() {
        format!("Specs ({} matches) — filter: {}", app.visible.len(), app.filter)
    } else {
        format!("Specs ({} items)", app.visible.len())
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.list_state);
}

fn draw_detail(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    let title;

    if let Some(node) = app.current_node() {
        if let Some(doc_idx) = node.doc_idx {
            let doc = &app.registry.documents[doc_idx];
            title = doc.id_str();

            lines.push(Line::from(vec![
                Span::styled("id      ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    doc.id_str(),
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("type    ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    doc.universal.entity_type.type_name().to_string(),
                    type_style(doc.universal.entity_type.prefix()),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("status  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    doc.universal.status.as_str().to_string(),
                    Style::default().fg(status_color(doc.universal.status)),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("version ", Style::default().fg(Color::DarkGray)),
                Span::raw(doc.universal.version.clone()),
            ]));
            if !doc.universal.owners.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("owners  ", Style::default().fg(Color::DarkGray)),
                    Span::raw(doc.universal.owners.join(", ")),
                ]));
            }
            lines.push(Line::from(vec![
                Span::styled("path    ", Style::default().fg(Color::DarkGray)),
                Span::raw(
                    doc.source_path
                        .strip_prefix(&app.registry.specs_dir)
                        .unwrap_or(&doc.source_path)
                        .display()
                        .to_string(),
                ),
            ]));

            // Type-specific fields
            match &doc.type_fields {
                TypeSpecificFields::Requirement {
                    level,
                    refines,
                    aspects,
                    categorized_under,
                    ..
                } => {
                    lines.push(Line::from(vec![
                        Span::styled("level   ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            level.as_str().to_string(),
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    if !refines.is_empty() {
                        lines.push(Line::from(Span::styled(
                            "refines",
                            Style::default().fg(Color::DarkGray),
                        )));
                        for r in refines {
                            lines.push(Line::from(format!("  ↳ {r}")));
                        }
                    }
                    if !aspects.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled("aspects ", Style::default().fg(Color::DarkGray)),
                            Span::raw(aspects.join(", ")),
                        ]));
                    }
                    if !categorized_under.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled("under   ", Style::default().fg(Color::DarkGray)),
                            Span::raw(categorized_under.join(", ")),
                        ]));
                    }
                }
                TypeSpecificFields::Task {
                    progress,
                    refines,
                    assignee,
                    eta,
                    blocked_by,
                    ..
                } => {
                    let (glyph, name) = progress_glyph(*progress);
                    lines.push(Line::from(vec![
                        Span::styled("progress", Style::default().fg(Color::DarkGray)),
                        Span::raw(" "),
                        Span::styled(format!("{glyph} {name}"), progress_style(*progress)),
                    ]));
                    if let Some(a) = assignee {
                        lines.push(Line::from(vec![
                            Span::styled("assignee", Style::default().fg(Color::DarkGray)),
                            Span::raw(format!(" {a}")),
                        ]));
                    }
                    if let Some(e) = eta {
                        lines.push(Line::from(vec![
                            Span::styled("eta     ", Style::default().fg(Color::DarkGray)),
                            Span::raw(e.clone()),
                        ]));
                    }
                    if !refines.is_empty() {
                        lines.push(Line::from(Span::styled(
                            "refines",
                            Style::default().fg(Color::DarkGray),
                        )));
                        for r in refines {
                            lines.push(Line::from(format!("  ↳ {r}")));
                        }
                    }
                    if !blocked_by.is_empty() {
                        lines.push(Line::from(Span::styled(
                            "blocked-by",
                            Style::default().fg(Color::DarkGray),
                        )));
                        for r in blocked_by {
                            lines.push(Line::from(format!("  ✋ {r}")));
                        }
                    }
                }
                TypeSpecificFields::Interface { stability, .. } => {
                    lines.push(Line::from(vec![
                        Span::styled("stable  ", Style::default().fg(Color::DarkGray)),
                        Span::raw(format!("{stability:?}")),
                    ]));
                }
                TypeSpecificFields::Adr {
                    decision_date,
                    decided_by,
                } => {
                    lines.push(Line::from(vec![
                        Span::styled("decided ", Style::default().fg(Color::DarkGray)),
                        Span::raw(decision_date.clone()),
                    ]));
                    if !decided_by.is_empty() {
                        lines.push(Line::from(vec![
                            Span::styled("by      ", Style::default().fg(Color::DarkGray)),
                            Span::raw(decided_by.join(", ")),
                        ]));
                    }
                }
                _ => {}
            }

            // Summary
            if let Some(summary) = &doc.universal.summary {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Summary",
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                )));
                for l in summary.lines() {
                    lines.push(Line::from(l.to_string()));
                }
            }

            // Children / ancestors
            let id_str = doc.id_str();
            let children = graph::query::children(&app.registry, &id_str);
            let ancestors = graph::query::ancestors(&app.registry, &id_str);
            if !ancestors.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Refined by",
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                )));
                for a in &ancestors {
                    lines.push(Line::from(format!("  ↑ {a}")));
                }
            }
            if !children.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Children",
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                )));
                for c in &children {
                    lines.push(Line::from(format!("  ↓ {c}")));
                }
            }

            // Clauses
            if !doc.blocks.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Blocks",
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                )));
                for b in &doc.blocks {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            b.kind.tag().to_string(),
                            Style::default().fg(Color::Blue),
                        ),
                        Span::raw(format!(" #{}", b.id)),
                    ]));
                    for c in &b.clauses {
                        lines.push(Line::from(format!("    · #{}  {}", c.id, truncate(&c.text, 70))));
                    }
                }
            }
        } else {
            // Group node — show count
            title = node.label.clone();
            let count = match node.kind {
                NodeKind::Namespace => app
                    .registry
                    .documents
                    .iter()
                    .filter(|d| d.universal.id.namespace == node.namespace)
                    .count(),
                NodeKind::Type => app
                    .registry
                    .documents
                    .iter()
                    .filter(|d| {
                        d.universal.id.namespace == node.namespace
                            && d.universal.entity_type.prefix() == node.entity_type.unwrap_or("")
                    })
                    .count(),
                _ => 0,
            };
            lines.push(Line::from(format!("{count} spec(s)")));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press l/→/Enter to expand, h/← to collapse.",
                Style::default().fg(Color::DarkGray),
            )));
        }
    } else {
        title = String::from("Detail");
        lines.push(Line::from("(no selection)"));
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn draw_status(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let text = if app.filter_mode {
        format!("/{}_  [Enter to apply, Esc to cancel]", app.filter)
    } else {
        "j/k: move   ^d/^u: half-page   ^e/^y: scroll   h/l: collapse/expand   /: filter   q: quit".to_string()
    };
    let p = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
    f.render_widget(p, area);
}

fn type_style(ty: &str) -> Style {
    let base = Style::default().add_modifier(Modifier::BOLD);
    match ty {
        "REQ" => base.fg(Color::Green),
        "INV" => base.fg(Color::Magenta),
        "IFC" => base.fg(Color::Blue),
        "ADR" => base.fg(Color::Yellow),
        "GLO" => base.fg(Color::White),
        "TOPIC" => base.fg(Color::Cyan),
        "SCN" => base.fg(Color::LightBlue),
        "TASK" => base.fg(Color::LightYellow),
        _ => base,
    }
}

fn progress_style(progress: Progress) -> Style {
    let base = Style::default().add_modifier(Modifier::BOLD);
    match progress {
        Progress::Done => base.fg(Color::Green),
        Progress::InProgress => base.fg(Color::Cyan),
        Progress::Blocked => base.fg(Color::Red),
        Progress::Pending => base.fg(Color::Yellow),
        Progress::Deferred => Style::default().fg(Color::DarkGray),
        Progress::WontDo => Style::default().fg(Color::DarkGray),
    }
}

fn status_color(status: Status) -> Color {
    match status {
        Status::Accepted => Color::Green,
        Status::Draft => Color::Yellow,
        Status::Deprecated => Color::DarkGray,
        Status::Superseded => Color::DarkGray,
    }
}

fn truncate(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}
