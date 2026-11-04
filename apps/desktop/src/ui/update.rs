//! Rendering for the "new version available" popup (see `crate::updates`).
use super::themed;
use crate::app::{App, HitTarget};
use crate::updates::BUTTONS;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap},
};
use vidcue_services::update::Release;

pub fn render(f: &mut Frame, app: &mut App, release: &Release, selected: usize, area: Rect) {
    let p = app.palette.clone();
    let colour = |c| themed(app, c);
    let (accent, muted, text, success, warning, selection, background) = (
        colour(p.accent),
        colour(p.muted),
        colour(p.text),
        colour(p.success),
        colour(p.warning),
        colour(p.selection),
        colour(p.background),
    );
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Line::from(vec![
            Span::styled(" ✦ ", Style::default().fg(warning)),
            Span::styled(
                app.tr("New version available"),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_bottom(
            Line::from(app.tr(" ←/→ choose · Enter confirms · u updates · Esc later ")).centered(),
        );
    let inner = block.inner(area).inner(Margin::new(2, 1));
    f.render_widget(block, area);
    let [body, buttons] =
        Layout::vertical([Constraint::Min(3), Constraint::Length(3)]).areas(inner);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", env!("CARGO_PKG_VERSION")),
                Style::default().fg(muted),
            ),
            Span::styled("➜ ", Style::default().fg(warning)),
            Span::styled(
                format!("{} ", release.version),
                Style::default().fg(success).add_modifier(Modifier::BOLD),
            ),
            Span::styled(&release.name, Style::default().fg(text)),
        ]),
        Line::from(""),
    ];
    lines.extend(release.notes.iter().map(|note| {
        Line::from(vec![
            Span::styled(" • ", Style::default().fg(accent)),
            Span::styled(note.as_str(), Style::default().fg(text)),
        ])
    }));
    lines.push(Line::from(""));
    lines.push(Line::styled(
        app.tr("Update now rebuilds this release from GitHub and replaces the vidcue command. Your settings and library stay."),
        Style::default().fg(muted),
    ));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body);
    let widths = BUTTONS.map(|b| Constraint::Length(app.tr(b).chars().count() as u16 + 6));
    let cells = Layout::horizontal(widths).spacing(2).split(buttons);
    for (index, (label, cell)) in BUTTONS.iter().zip(cells.iter()).enumerate() {
        let chosen = index == selected;
        let colour = if index == 0 { success } else { muted };
        let style = if chosen {
            Style::default()
                .fg(background)
                .bg(colour)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colour).bg(selection)
        };
        f.render_widget(
            Paragraph::new(format!(
                "{}{}",
                if chosen { "▸ " } else { "  " },
                app.tr(label)
            ))
            .style(style)
            .centered()
            .block(Block::default().padding(Padding::top(1))),
            *cell,
        );
        app.hit_targets.push((*cell, HitTarget::MenuItem(index)));
    }
}
