use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{prelude as Ra, widgets as Raw};

use super::utils::SharedTheme;
use crate::ui::EventState;
// use crate::visible_methods;

pub struct ClashTuiTabBar {
    title: String,
    is_visible: bool,
    pub tab_titles: Vec<String>,
    pub index: usize,

    pub theme: SharedTheme,
}

impl ClashTuiTabBar {
    pub fn new(title: String, tab_titles: Vec<String>, theme: SharedTheme) -> Self {
        Self {
            title,
            is_visible: true,
            tab_titles,
            index: 0,

            theme,
        }
    }

    pub fn event(&mut self, ev: &Event) -> Result<EventState, std::io::Error> {
        if !self.is_visible {
            return Ok(EventState::NotConsumed);
        }

        let mut event_stata = EventState::NotConsumed;
        if let Event::Key(key) = ev {
            if key.kind == KeyEventKind::Press {
                event_stata = match key.code {
                    // 1..=9
                    KeyCode::Char(c) if c.is_digit(10) && c != '0' => {
                        let digit = c.to_digit(10);
                        if let Some(d) = digit {
                            if d <= self.tab_titles.len() as u32 {
                                self.index = (d - 1) as usize;
                            }
                        }
                        EventState::WorkDone
                    }
                    _ => EventState::NotConsumed,
                }
            }
        }

        Ok(event_stata)
    }

    pub fn draw<B: Ra::Backend>(&mut self, f: &mut Ra::Frame<B>, area: Ra::Rect) {
        let items = self
            .tab_titles
            .iter()
            .map(|t| {
                Ra::text::Line::from(Ra::Span::styled(
                    t,
                    Ra::Style::default().fg(self.theme.tab_txt_fg),
                ))
            })
            .collect();
        let tabs = Raw::Tabs::new(items)
            .block(
                Raw::Block::default()
                    .borders(Raw::Borders::ALL)
                    .title(self.title.clone()),
            )
            .highlight_style(Ra::Style::default().fg(self.theme.tab_hl_fg))
            .select(self.index);
        f.render_widget(tabs, area);
    }

    pub fn next(&mut self) {
        self.index = (self.index + 1) % self.tab_titles.len();
    }

    pub fn previous(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        } else {
            self.index = self.tab_titles.len() - 1;
        }
    }

    pub fn selected(&self) -> Option<&String> {
        return Some(&self.tab_titles[self.index]);
    }
}

// visible_methods!(ClashTuiTabBar);