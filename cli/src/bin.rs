use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, Wrap},
};
use std::io;
use std::time::Duration;
use ttp_core::{CommunicationValue, DataTypes, DataValue};

pub struct BinaryToCommApp {
    binary_input: String,
    binary_error: Option<String>,
    parsed_value: Option<CommunicationValue>,
    selected_tab: usize,
    scroll_offset: usize,
    show_help: bool,
}

impl BinaryToCommApp {
    pub fn new() -> Self {
        Self {
            binary_input: String::new(),
            binary_error: None,
            parsed_value: None,
            selected_tab: 0,
            scroll_offset: 0,
            show_help: false,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
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
    ) -> io::Result<()> {
        let mut last_tick = std::time::Instant::now();
        let tick_rate = Duration::from_millis(250);

        loop {
            terminal.draw(|f| self.draw(f)).unwrap();

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('h') => self.show_help = !self.show_help,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(());
                        }
                        KeyCode::Char(c) => {
                            if c.is_ascii_hexdigit() || c == ',' || c == ' ' || c == '[' || c == ']'
                            {
                                self.binary_input.push(c);
                                self.try_parse();
                            }
                        }
                        KeyCode::Backspace => {
                            self.binary_input.pop();
                            self.try_parse();
                        }
                        KeyCode::Tab => {
                            self.selected_tab = (self.selected_tab + 1) % 2;
                        }
                        KeyCode::Up => {
                            if self.scroll_offset > 0 {
                                self.scroll_offset -= 1;
                            }
                        }
                        KeyCode::Down => {
                            self.scroll_offset += 1;
                        }
                        _ => {}
                    }
                }
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = std::time::Instant::now();
            }
        }
    }

    fn try_parse(&mut self) {
        self.binary_error = None;
        self.parsed_value = None;

        let cleaned: String = self
            .binary_input
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .collect();

        if cleaned.len() % 2 != 0 {
            self.binary_error = Some("Incomplete byte (odd number of hex digits)".to_string());
            return;
        }

        let bytes: Result<Vec<u8>, _> = (0..cleaned.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16))
            .collect();

        match bytes {
            Ok(bytes) => {
                if bytes.is_empty() {
                    self.binary_error = Some("No bytes to parse".to_string());
                    return;
                }
                match CommunicationValue::from_bytes(&bytes) {
                    Some(cv) => self.parsed_value = Some(cv),
                    None => {
                        self.binary_error = Some("Failed to parse CommunicationValue".to_string())
                    }
                }
            }
            Err(_) => {
                self.binary_error = Some("Invalid hex format".to_string());
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(frame.area());

        let header_block = Block::default()
            .title(" Binary → CommunicationValue Converter ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let input_text = if self.binary_input.is_empty() {
            Text::from(vec![Line::from(vec![
                Span::styled("Enter hex bytes: ", Style::default().fg(Color::Gray)),
                Span::styled("[1c, 20, 99, ...]", Style::default().fg(Color::DarkGray)),
            ])])
        } else {
            Text::from(vec![Line::from(vec![
                Span::styled("Hex: ", Style::default().fg(Color::Green)),
                Span::styled(&self.binary_input, Style::default().fg(Color::White)),
            ])])
        };

        let input_para = Paragraph::new(input_text).block(header_block);
        frame.render_widget(input_para, chunks[0]);

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(chunks[1]);

        self.draw_bytes_view(frame, main_chunks[0]);
        self.draw_structure_view(frame, main_chunks[1]);

        if self.show_help {
            self.draw_help(frame);
        }
    }

    fn draw_bytes_view(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Raw Bytes ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.binary_input.is_empty() {
            let text = Paragraph::new("Enter hex bytes above...")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(text, inner);
            return;
        }

        let cleaned: String = self
            .binary_input
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .collect();

        let bytes: Vec<u8> = cleaned
            .as_bytes()
            .chunks(2)
            .filter_map(|chunk| {
                if chunk.len() == 2 {
                    std::str::from_utf8(chunk)
                        .ok()
                        .and_then(|s| u8::from_str_radix(s, 16).ok())
                } else {
                    None
                }
            })
            .collect();
        let mut lines = vec![];

        // Header
        lines.push(Line::from(vec![
            Span::styled("Offset ", Style::default().fg(Color::Yellow)),
            Span::styled(
                " 00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F",
                Style::default().fg(Color::Yellow),
            ),
            Span::styled("  ASCII", Style::default().fg(Color::Yellow)),
        ]));

        // Calculate byte positions for coloring
        let mut byte_idx = 0;

        for (row_idx, chunk) in bytes.chunks(16).enumerate() {
            let offset = row_idx * 16;
            let mut hex_spans: Vec<Span> = vec![];
            let mut ascii_part = String::new();

            for (i, byte) in chunk.iter().enumerate() {
                if i > 0 && i % 2 == 0 {
                    hex_spans.push(Span::raw(" "));
                }

                // Determine color based on byte position
                let color = if byte_idx == 0 {
                    Color::Magenta // Communication Type
                } else if byte_idx == 1 {
                    Color::Cyan // Flags byte (has_sender, has_receiver, has_id)
                } else if let Some(ref cv) = self.parsed_value {
                    let has_sender = cv.get_sender() != 0;
                    let has_receiver = cv.get_receiver() != 0;
                    let has_id = cv.get_id() != 0;

                    let mut pos = 2; // After type and flags

                    if has_id {
                        if byte_idx >= pos && byte_idx < pos + 4 {
                            Color::Yellow // ID (4 bytes)
                        } else {
                            pos += 4;
                            if has_sender && byte_idx >= pos && byte_idx < pos + 6 {
                                Color::Green // Sender (6 bytes)
                            } else {
                                pos += if has_sender { 6 } else { 0 };
                                if has_receiver && byte_idx >= pos && byte_idx < pos + 6 {
                                    Color::Blue // Receiver (6 bytes)
                                } else {
                                    pos += if has_receiver { 6 } else { 0 };
                                    if byte_idx >= pos + 4 {
                                        Color::Red // Data container (after length)
                                    } else {
                                        Color::White // Data length (4 bytes)
                                    }
                                }
                            }
                        }
                    } else {
                        if has_sender && byte_idx >= pos && byte_idx < pos + 6 {
                            Color::Green // Sender
                        } else {
                            pos += if has_sender { 6 } else { 0 };
                            if has_receiver && byte_idx >= pos && byte_idx < pos + 6 {
                                Color::Blue // Receiver
                            } else {
                                pos += if has_receiver { 6 } else { 0 };
                                if byte_idx >= pos + 4 {
                                    Color::Red // Data container
                                } else {
                                    Color::White // Data length
                                }
                            }
                        }
                    }
                } else {
                    Color::White // Default if not parsed
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

        // Legend
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

        let para = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true });
        frame.render_widget(para, inner);
    }

    fn draw_structure_view(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Parsed Structure ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if let Some(ref cv) = self.parsed_value {
            let mut lines = vec![];

            lines.push(Line::from(vec![
                Span::styled(
                    "Communication Type: ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:?}", cv.get_type()),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(" (0x{:02x})", cv.get_type().as_number()),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("Sender: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{}", cv.get_sender()),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(" (0x{:012x})", cv.get_sender()),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("Receiver: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{}", cv.get_receiver()),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(" (0x{:012x})", cv.get_receiver()),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{}", cv.get_id()),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(" (0x{:08x})", cv.get_id()),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Data Container:",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )]));

            self.render_container(&mut lines, cv.get_data_container(), 0);

            let para = Paragraph::new(Text::from(lines))
                .scroll((self.scroll_offset as u16, 0))
                .wrap(Wrap { trim: true });
            frame.render_widget(para, inner);

            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            frame.render_stateful_widget(
                scrollbar,
                inner.inner(Margin {
                    horizontal: 0,
                    vertical: 0,
                }),
                &mut ratatui::widgets::ScrollbarState::new(100).position(self.scroll_offset),
            );
        } else if let Some(ref err) = self.binary_error {
            let text = Paragraph::new(Text::from(vec![
                Line::from(vec![Span::styled(
                    "Error:",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )]),
                Line::from(err.clone()),
            ]))
            .style(Style::default().fg(Color::Red));
            frame.render_widget(text, inner);
        } else {
            let text = Paragraph::new("Waiting for valid input...")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(text, inner);
        }
    }

    fn render_container(
        &self,
        lines: &mut Vec<Line>,
        container: &std::collections::BTreeMap<DataTypes, DataValue>,
        indent: usize,
    ) {
        let indent_str = "  ".repeat(indent);

        if container.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{}└─ ", indent_str),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("(empty)", Style::default().fg(Color::DarkGray)),
            ]));
            return;
        }

        for (idx, (key, value)) in container.iter().enumerate() {
            let is_last = idx == container.len() - 1;
            let branch = if is_last { "└─ " } else { "├─ " };

            match value {
                DataValue::Number(n) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(format!("{:?}: ", key), Style::default().fg(Color::Yellow)),
                        Span::styled(n.to_string(), Style::default().fg(Color::Green)),
                    ]));
                }
                DataValue::Str(s) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(format!("{:?}: ", key), Style::default().fg(Color::Yellow)),
                        Span::styled(format!("\"{}\"", s), Style::default().fg(Color::Green)),
                    ]));
                }
                DataValue::Bool(b) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(format!("{:?}: ", key), Style::default().fg(Color::Yellow)),
                        Span::styled(
                            b.to_string(),
                            Style::default().fg(if *b { Color::Green } else { Color::Red }),
                        ),
                    ]));
                }
                DataValue::BoolTrue => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(format!("{:?}: ", key), Style::default().fg(Color::Yellow)),
                        Span::styled("true", Style::default().fg(Color::Green)),
                    ]));
                }
                DataValue::BoolFalse => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(format!("{:?}: ", key), Style::default().fg(Color::Yellow)),
                        Span::styled("false", Style::default().fg(Color::Red)),
                    ]));
                }
                DataValue::Null => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(format!("{:?}: ", key), Style::default().fg(Color::Yellow)),
                        Span::styled("null", Style::default().fg(Color::DarkGray)),
                    ]));
                }
                DataValue::Array(arr) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(format!("{:?}: ", key), Style::default().fg(Color::Yellow)),
                        Span::styled(
                            format!("[{} items]", arr.len()),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]));
                    self.render_array(lines, arr, indent + 1);
                }
                DataValue::Container(inner) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(format!("{:?}: ", key), Style::default().fg(Color::Yellow)),
                        Span::styled("{...}", Style::default().fg(Color::Cyan)),
                    ]));
                    let map: std::collections::BTreeMap<DataTypes, DataValue> =
                        inner.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    self.render_container(lines, &map, indent + 1);
                }
            }
        }
    }

    fn render_array(&self, lines: &mut Vec<Line>, arr: &[DataValue], indent: usize) {
        let indent_str = "  ".repeat(indent);

        for (idx, value) in arr.iter().enumerate() {
            let is_last = idx == arr.len() - 1;
            let branch = if is_last { "└─ " } else { "├─ " };
            let idx_str = format!("[{}]: ", idx);

            match value {
                DataValue::Number(n) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(idx_str, Style::default().fg(Color::DarkGray)),
                        Span::styled(n.to_string(), Style::default().fg(Color::Green)),
                    ]));
                }
                DataValue::Str(s) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(idx_str, Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("\"{}\"", s), Style::default().fg(Color::Green)),
                    ]));
                }
                DataValue::Container(inner) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(idx_str, Style::default().fg(Color::DarkGray)),
                        Span::styled("{...}", Style::default().fg(Color::Cyan)),
                    ]));
                    let map: std::collections::BTreeMap<DataTypes, DataValue> =
                        inner.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    self.render_container(lines, &map, indent + 1);
                }
                DataValue::Array(inner_arr) => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(idx_str, Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            format!("[{} items]", inner_arr.len()),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]));
                    self.render_array(lines, inner_arr, indent + 1);
                }
                _ => {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("{}{}", indent_str, branch),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(idx_str, Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{:?}", value), Style::default().fg(Color::White)),
                    ]));
                }
            }
        }
    }

    fn draw_help(&self, frame: &mut Frame) {
        let area = centered_rect(60, 40, frame.area());

        // Clear background first
        frame.render_widget(Clear, area);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().bg(Color::Black));

        let text = Text::from(vec![
            Line::from(vec![Span::styled(
                "Controls:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from("  Type hex bytes: [1c, 20, 99, 6c, ...] or 1c20996c..."),
            Line::from("  Backspace      - Delete last character"),
            Line::from("  Tab            - Switch views"),
            Line::from("  ↑/↓            - Scroll structure view"),
            Line::from("  h              - Toggle this help"),
            Line::from("  q/Esc          - Quit"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Format:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from("  Accepts: comma-separated hex, spaces, brackets"),
            Line::from("  Example: [1c, 20, 99, 6c, dd, 6e, 00, 00]"),
        ]);

        let para = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
        frame.render_widget(para, area);
    }
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

pub fn run_binary_to_comm_converter() -> io::Result<()> {
    let mut app = BinaryToCommApp::new();
    app.run()
}
