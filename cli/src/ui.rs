use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, Event, KeyCode, KeyModifiers},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    prelude::Backend,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, Wrap,
    },
};
use std::io::{self, Error};
use std::time::Duration;
use strum::IntoEnumIterator;
use ttp_core::{CommunicationType, CommunicationValue, DataKind, DataTypes, DataValue};

pub struct CommToBinaryApp {
    comm_type: CommunicationType,
    sender: String,
    receiver: String,
    id: String,
    data_entries: Vec<(DataTypes, DataValue)>,
    current_key: Option<DataTypes>,
    current_value: String,
    selected_field: usize,
    edit_mode: bool,
    show_comm_type_list: bool,
    show_data_type_list: bool,
    comm_list_state: ListState,
    data_type_list_state: ListState,
    scroll_offset: usize,
    show_help: bool,
    generated_bytes: Option<Vec<u8>>,
    status_message: Option<String>,
}

impl CommToBinaryApp {
    pub fn new() -> Self {
        let mut comm_list_state = ListState::default();
        comm_list_state.select(Some(0));

        let mut data_type_list_state = ListState::default();
        data_type_list_state.select(Some(0));

        Self {
            comm_type: CommunicationType::ping,
            sender: "0".to_string(),
            receiver: "0".to_string(),
            id: "0".to_string(),
            data_entries: vec![],
            current_key: None,
            current_value: String::new(),
            selected_field: 0,
            edit_mode: false,
            show_comm_type_list: false,
            show_data_type_list: false,
            comm_list_state,
            data_type_list_state,
            scroll_offset: 0,
            show_help: false,
            generated_bytes: None,
            status_message: None,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_app(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn run_app<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<(), Error>
    where
        std::io::Error: From<<B as Backend>::Error>,
    {
        let tick_rate = Duration::from_millis(250);
        let mut last_tick = std::time::Instant::now();

        loop {
            terminal.draw(|f| self.draw(f))?;

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    // Clear status message on any key press
                    self.status_message = None;

                    if self.show_comm_type_list {
                        self.handle_comm_list_input(key.code);
                    } else if self.show_data_type_list {
                        self.handle_data_type_list_input(key.code);
                    } else if self.edit_mode {
                        self.handle_edit_input(key.code, key.modifiers);
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char('h') => self.show_help = !self.show_help,
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(());
                            }
                            KeyCode::Tab | KeyCode::Down => {
                                self.selected_field = (self.selected_field + 1) % 6;
                            }
                            KeyCode::BackTab | KeyCode::Up => {
                                if self.selected_field == 0 {
                                    self.selected_field = 5;
                                } else {
                                    self.selected_field -= 1;
                                }
                            }
                            KeyCode::Enter => {
                                match self.selected_field {
                                    0 => self.show_comm_type_list = true,
                                    4 => {
                                        // If we have a key, enter edit mode to type value
                                        // Otherwise, open the list to select a key
                                        if self.current_key.is_some() {
                                            self.edit_mode = true;
                                        } else {
                                            self.show_data_type_list = true;
                                        }
                                    }
                                    5 => self.generate_binary(),
                                    _ => self.edit_mode = true,
                                }
                            }
                            KeyCode::Char('d') => {
                                if self.selected_field == 4 && !self.data_entries.is_empty() {
                                    self.data_entries.pop();
                                    self.generate_binary();
                                    self.status_message = Some("Last entry deleted".to_string());
                                }
                            }
                            KeyCode::Char('a') => {
                                if self.selected_field == 4 {
                                    self.add_current_entry();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = std::time::Instant::now();
            }
        }
    }

    fn add_current_entry(&mut self) {
        if let Some(key) = self.current_key.clone() {
            if !self.current_value.is_empty() {
                if let Some(value) = self.parse_value_for_key(&key, &self.current_value) {
                    self.data_entries.push((key, value));
                    self.current_key = None;
                    self.current_value.clear();
                    self.generate_binary();
                    self.status_message = Some("Entry added!".to_string());
                } else {
                    self.status_message = Some("Invalid value for this DataKind".to_string());
                }
            } else {
                self.status_message = Some("Type a value first".to_string());
            }
        } else {
            self.status_message = Some("Select a key first (press Enter)".to_string());
        }
    }

    fn handle_comm_list_input(&mut self, key: KeyCode) {
        let comm_types: Vec<_> = CommunicationType::iter().collect();

        match key {
            KeyCode::Esc => self.show_comm_type_list = false,
            KeyCode::Enter => {
                if let Some(idx) = self.comm_list_state.selected() {
                    self.comm_type = comm_types[idx].clone();
                }
                self.show_comm_type_list = false;
                self.generate_binary();
            }
            KeyCode::Up => {
                let idx = self.comm_list_state.selected().unwrap_or(0);
                if idx > 0 {
                    self.comm_list_state.select(Some(idx - 1));
                }
            }
            KeyCode::Down => {
                let idx = self.comm_list_state.selected().unwrap_or(0);
                if idx < comm_types.len() - 1 {
                    self.comm_list_state.select(Some(idx + 1));
                }
            }
            _ => {}
        }
    }

    fn handle_data_type_list_input(&mut self, key: KeyCode) {
        let data_types: Vec<_> = DataTypes::iter().collect();

        match key {
            KeyCode::Esc => self.show_data_type_list = false,
            KeyCode::Enter => {
                if let Some(idx) = self.data_type_list_state.selected() {
                    self.current_key = Some(data_types[idx].clone());
                    self.show_data_type_list = false;
                    self.current_value.clear();
                    // Automatically enter edit mode to type the value
                    self.edit_mode = true;
                }
            }
            KeyCode::Up => {
                let idx = self.data_type_list_state.selected().unwrap_or(0);
                if idx > 0 {
                    self.data_type_list_state.select(Some(idx - 1));
                }
            }
            KeyCode::Down => {
                let idx = self.data_type_list_state.selected().unwrap_or(0);
                if idx < data_types.len() - 1 {
                    self.data_type_list_state.select(Some(idx + 1));
                }
            }
            _ => {}
        }
    }

    #[allow(unused_variables)]
    fn handle_edit_input(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        match key {
            KeyCode::Esc => self.edit_mode = false,
            KeyCode::Enter => {
                self.edit_mode = false;
                self.generate_binary();
            }
            KeyCode::Char(c) => match self.selected_field {
                1 => self.sender.push(c),
                2 => self.receiver.push(c),
                3 => self.id.push(c),
                4 => {
                    // When in edit mode on field 4, type into current_value
                    self.current_value.push(c);
                }
                _ => {}
            },
            KeyCode::Backspace => match self.selected_field {
                1 => {
                    self.sender.pop();
                }
                2 => {
                    self.receiver.pop();
                }
                3 => {
                    self.id.pop();
                }
                4 => {
                    self.current_value.pop();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn parse_value_for_key(&self, key: &DataTypes, s: &str) -> Option<DataValue> {
        let kind = key.expected_kind();

        match kind {
            DataKind::Number => s.parse::<i64>().ok().map(DataValue::Number),
            DataKind::Str => Some(DataValue::Str(s.to_string())),
            DataKind::Bool => {
                if s.eq_ignore_ascii_case("true") || s == "1" {
                    Some(DataValue::Bool(true))
                } else if s.eq_ignore_ascii_case("false") || s == "0" {
                    Some(DataValue::Bool(false))
                } else {
                    None
                }
            }
            DataKind::Null => Some(DataValue::Null),
            DataKind::Array(inner_kind) => match *inner_kind {
                DataKind::Number => {
                    let nums: Result<Vec<i64>, _> =
                        s.split(',').map(str::trim).map(str::parse).collect();
                    nums.ok()
                        .map(|v| DataValue::Array(v.into_iter().map(DataValue::Number).collect()))
                }
                DataKind::Str => {
                    let strs: Vec<String> = s.split(',').map(|s| s.trim().to_string()).collect();
                    Some(DataValue::Array(
                        strs.into_iter().map(DataValue::Str).collect(),
                    ))
                }
                _ => None,
            },
            DataKind::Container => None,
        }
    }

    fn generate_binary(&mut self) {
        let mut cv = CommunicationValue::new(self.comm_type.clone());

        if let Ok(id) = self.id.parse::<u32>() {
            if id != 0 {
                cv = cv.with_id(id);
            }
        }

        if let Ok(sender) = self.sender.parse::<u64>() {
            if sender != 0 {
                cv = cv.with_sender(sender);
            }
        }

        if let Ok(receiver) = self.receiver.parse::<u64>() {
            if receiver != 0 {
                cv = cv.with_receiver(receiver);
            }
        }

        for (key, value) in &self.data_entries {
            cv = cv.add_data(key.clone(), value.clone());
        }

        if let Some(ref key) = self.current_key {
            if !self.current_value.is_empty() {
                if let Some(value) = self.parse_value_for_key(key, &self.current_value) {
                    cv = cv.add_data(key.clone(), value);
                }
            }
        }

        self.generated_bytes = Some(cv.to_bytes());
    }

    fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(16), Constraint::Min(0)]) // Increased height for status
            .split(frame.area());

        self.draw_form(frame, chunks[0]);
        self.draw_output(frame, chunks[1]);

        if self.show_comm_type_list {
            let area = centered_rect(50, 70, frame.area());
            frame.render_widget(Clear, area);
            self.draw_comm_type_list(frame, area);
        } else if self.show_data_type_list {
            let area = centered_rect(50, 70, frame.area());
            frame.render_widget(Clear, area);
            self.draw_data_type_list(frame, area);
        } else if self.show_help {
            let area = centered_rect(60, 50, frame.area());
            frame.render_widget(Clear, area);
            self.draw_help(frame, area);
        }
    }

    fn draw_form(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" CommunicationValue → Binary Builder ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let fields = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(5),
                Constraint::Length(1),
                Constraint::Length(1), // Status line
            ])
            .split(inner);

        // Communication Type
        let is_selected = self.selected_field == 0 && !self.edit_mode;
        let style = if is_selected {
            Style::default().bg(Color::DarkGray).fg(Color::Black)
        } else {
            Style::default().fg(Color::White)
        };
        let comm_text = Line::from(vec![
            Span::styled("Type:     ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:?}", self.comm_type), style),
            Span::styled(" [Enter to change]", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(comm_text), fields[0]);

        // Sender
        let is_selected = self.selected_field == 1 || (self.selected_field == 1 && self.edit_mode);
        let style = if is_selected {
            Style::default().bg(Color::DarkGray).fg(Color::Black)
        } else {
            Style::default().fg(Color::White)
        };
        let sender_text = Line::from(vec![
            Span::styled("Sender:   ", Style::default().fg(Color::Cyan)),
            Span::styled(&self.sender, style),
        ]);
        frame.render_widget(Paragraph::new(sender_text), fields[1]);

        // Receiver
        let is_selected = self.selected_field == 2 || (self.selected_field == 2 && self.edit_mode);
        let style = if is_selected {
            Style::default().bg(Color::DarkGray).fg(Color::Black)
        } else {
            Style::default().fg(Color::White)
        };
        let receiver_text = Line::from(vec![
            Span::styled("Receiver: ", Style::default().fg(Color::Cyan)),
            Span::styled(&self.receiver, style),
        ]);
        frame.render_widget(Paragraph::new(receiver_text), fields[2]);

        // ID
        let is_selected = self.selected_field == 3 || (self.selected_field == 3 && self.edit_mode);
        let style = if is_selected {
            Style::default().bg(Color::DarkGray).fg(Color::Black)
        } else {
            Style::default().fg(Color::White)
        };
        let id_text = Line::from(vec![
            Span::styled("ID:       ", Style::default().fg(Color::Cyan)),
            Span::styled(&self.id, style),
        ]);
        frame.render_widget(Paragraph::new(id_text), fields[3]);

        // Data entries
        let is_selected = self.selected_field == 4;
        let data_block = Block::default()
            .title(" Data Entries (Enter=add key, a=add entry, d=delete last) ")
            .borders(Borders::ALL)
            .border_style(if is_selected {
                Color::Yellow
            } else {
                Color::Gray
            });

        let data_inner = data_block.inner(fields[4]);
        frame.render_widget(data_block, fields[4]);

        let mut data_lines = vec![];

        for (key, value) in &self.data_entries {
            let kind = key.expected_kind();
            data_lines.push(Line::from(vec![
                Span::styled(format!("{:?}", key), Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!(" ({:?}): ", kind),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(format!("{:?}", value), Style::default().fg(Color::Green)),
            ]));
        }

        if let Some(ref key) = self.current_key {
            let kind = key.expected_kind();
            let edit_indicator = if self.edit_mode { " [EDIT]" } else { "" };
            data_lines.push(Line::from(vec![
                Span::styled(
                    format!("{:?}", key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::UNDERLINED),
                ),
                Span::styled(
                    format!(" ({:?}){}: ", kind, edit_indicator),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    &self.current_value,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::UNDERLINED),
                ),
            ]));
        } else if is_selected {
            data_lines.push(Line::from(vec![Span::styled(
                "Press Enter to select DataType key...",
                Style::default().fg(Color::DarkGray),
            )]));
        }

        if data_lines.is_empty() && self.current_key.is_none() {
            data_lines.push(Line::from(vec![Span::styled(
                "(no data entries)",
                Style::default().fg(Color::DarkGray),
            )]));
        }

        frame.render_widget(Paragraph::new(Text::from(data_lines)), data_inner);

        // Generate button
        let is_selected = self.selected_field == 5;
        let style = if is_selected {
            Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };
        let gen_text = Line::from(vec![Span::styled(
            ">>> Press Enter to Generate Binary <<<",
            style,
        )]);
        frame.render_widget(
            Paragraph::new(gen_text).alignment(Alignment::Center),
            fields[5],
        );

        // Status message
        if let Some(ref msg) = self.status_message {
            let status = Line::from(vec![Span::styled(
                msg,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]);
            frame.render_widget(
                Paragraph::new(status).alignment(Alignment::Center),
                fields[6],
            );
        }
    }

    fn draw_output(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Generated Binary Output ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if let Some(ref bytes) = self.generated_bytes {
            let hex_string = bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(", ");

            let formatted = format!("[{}]", hex_string);

            let mut lines = vec![
                Line::from(vec![Span::styled(
                    "Hex Format:",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    &formatted,
                    Style::default().fg(Color::White),
                )]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Byte Count: ", Style::default().fg(Color::Cyan)),
                    Span::styled(bytes.len().to_string(), Style::default().fg(Color::Yellow)),
                ]),
            ];

            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Detailed View:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]));

            let mut byte_idx = 0;

            for (idx, chunk) in bytes.chunks(16).enumerate() {
                let offset = idx * 16;
                let mut hex_spans: Vec<Span> = vec![];
                let mut ascii_part = String::new();

                for byte in chunk.iter() {
                    let color = if byte_idx == 0 {
                        Color::Magenta
                    } else if byte_idx == 1 {
                        Color::Cyan
                    } else {
                        let flags = bytes.get(1).copied().unwrap_or(0);
                        let has_id = (flags & 0b0010_0000) != 0;
                        let has_sender = (flags & 0b1000_0000) != 0;
                        let has_receiver = (flags & 0b0100_0000) != 0;

                        let mut pos = 2;

                        if has_id {
                            if byte_idx >= pos && byte_idx < pos + 4 {
                                Color::Yellow
                            } else {
                                pos += 4;
                                if has_sender && byte_idx >= pos && byte_idx < pos + 6 {
                                    Color::Green
                                } else {
                                    pos += if has_sender { 6 } else { 0 };
                                    if has_receiver && byte_idx >= pos && byte_idx < pos + 6 {
                                        Color::Blue
                                    } else {
                                        pos += if has_receiver { 6 } else { 0 };
                                        if byte_idx >= pos + 4 {
                                            Color::Red
                                        } else {
                                            Color::White
                                        }
                                    }
                                }
                            }
                        } else {
                            if has_sender && byte_idx >= pos && byte_idx < pos + 6 {
                                Color::Green
                            } else {
                                pos += if has_sender { 6 } else { 0 };
                                if has_receiver && byte_idx >= pos && byte_idx < pos + 6 {
                                    Color::Blue
                                } else {
                                    pos += if has_receiver { 6 } else { 0 };
                                    if byte_idx >= pos + 4 {
                                        Color::Red
                                    } else {
                                        Color::White
                                    }
                                }
                            }
                        }
                    };

                    hex_spans.push(Span::styled(
                        format!("{:02x} ", byte),
                        Style::default().fg(color),
                    ));

                    let c = if byte.is_ascii_graphic() || *byte == b' ' {
                        *byte as char
                    } else {
                        '.'
                    };
                    ascii_part.push(c);

                    byte_idx += 1;
                }

                let line = Line::from(
                    std::iter::once(Span::styled(
                        format!("{:04X}  ", offset),
                        Style::default().fg(Color::Yellow),
                    ))
                    .chain(hex_spans.into_iter())
                    .chain(std::iter::once(Span::styled(
                        format!("  {}", ascii_part),
                        Style::default().fg(Color::Green),
                    )))
                    .collect::<Vec<_>>(),
                );
                lines.push(line);
            }

            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Legend: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("Type ", Style::default().fg(Color::Magenta)),
                Span::styled("Flags ", Style::default().fg(Color::Cyan)),
                Span::styled("ID ", Style::default().fg(Color::Yellow)),
                Span::styled("Sender ", Style::default().fg(Color::Green)),
                Span::styled("Receiver ", Style::default().fg(Color::Blue)),
                Span::styled("DataLen ", Style::default().fg(Color::White)),
                Span::styled("Data", Style::default().fg(Color::Red)),
            ]));

            let para = Paragraph::new(Text::from(lines))
                .scroll((self.scroll_offset as u16, 0))
                .wrap(Wrap { trim: true });
            frame.render_widget(para, inner);

            let scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
            frame.render_stateful_widget(
                scrollbar,
                inner.inner(Margin {
                    horizontal: 0,
                    vertical: 0,
                }),
                &mut ratatui::widgets::ScrollbarState::new(100).position(self.scroll_offset),
            );
        } else {
            let text = Paragraph::new("Fill in fields and press Enter to generate...")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(text, inner);
        }
    }

    fn draw_comm_type_list(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Select Communication Type ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let items: Vec<_> = CommunicationType::iter()
            .map(|ct| {
                let style = if self.comm_type == ct {
                    Style::default().bg(Color::Green).fg(Color::Black)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{:?}", ct)).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, area, &mut self.comm_list_state);
    }

    fn draw_data_type_list(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Select Data Type Key ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let items: Vec<_> = DataTypes::iter()
            .map(|dt| {
                let kind = dt.expected_kind();
                ListItem::new(format!("{:?} ({:?})", dt, kind))
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, area, &mut self.data_type_list_state);
    }

    fn draw_help(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let text = Text::from(vec![
            Line::from(vec![Span::styled(
                "Navigation:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from("  Tab/↑/↓        - Select field"),
            Line::from("  Enter          - Edit field / Select from list"),
            Line::from("  Esc            - Cancel edit / Close list"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Fields:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from("  Type           - CommunicationType enum value"),
            Line::from("  Sender/Receiver- u64 IDs (0 = omitted)"),
            Line::from("  ID             - u32 message ID (0 = omitted)"),
            Line::from("  Data Entries   - Key-value pairs for container"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Data Entry Workflow:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from("  1. Press Enter to open DataType list"),
            Line::from("  2. Select key with Enter (auto-enters edit mode)"),
            Line::from("  3. Type value"),
            Line::from("  4. Press 'a' to add to entries list"),
            Line::from("  5. Repeat or press Enter to finish"),
            Line::from(""),
            Line::from("  d              - Delete last entry"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Output:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from("  Shows hex array format and detailed hex dump"),
            Line::from("  Bytes are color-coded by semantic meaning"),
        ]);

        let para = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
        frame.render_widget(para, area);
    }
}

pub fn run_comm_to_binary_converter() -> io::Result<()> {
    let mut app = CommToBinaryApp::new();
    app.run()
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
