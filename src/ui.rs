//! Pure state -> widget rendering. Nord palette, English UI.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Bar, BarChart, BarGroup, Block, Chart, Clear, Dataset, GraphType, Paragraph, Row, Table,
    TableState, Tabs,
};
use ratatui::Frame;

use crate::app::{App, Form, Mode, Tab};
use crate::money::format_minor;
use crate::report::{self, ym_label};

const ACCENT: Color = Color::Rgb(136, 192, 208); // Nord8
const OK: Color = Color::Rgb(163, 190, 140); // Nord14
const BAD: Color = Color::Rgb(191, 97, 106); // Nord11
const WARN: Color = Color::Rgb(235, 203, 139); // Nord13
const MUTED: Color = Color::DarkGray;

fn amount_style(value: i64) -> Style {
    Style::default().fg(if value < 0 { BAD } else { OK })
}

pub fn draw(frame: &mut Frame, app: &App) {
    let [header, tab_bar, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_header(frame, header, app);
    draw_tabs(frame, tab_bar, app);

    match app.tab {
        Tab::Dashboard => draw_dashboard(frame, body, app),
        Tab::Accounts => draw_accounts(frame, body, app),
        Tab::Register => draw_register(frame, body, app),
        Tab::Reports => draw_reports(frame, body, app),
    }

    draw_footer(frame, footer, app);

    match app.mode {
        Mode::Help => draw_help(frame, frame.area()),
        Mode::Form => {
            if let Some(form) = app.form.as_ref() {
                draw_form(frame, frame.area(), form);
            }
        }
        Mode::ConfirmDelete { .. } => draw_confirm(frame, frame.area()),
        Mode::Browse => {}
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let month = format!("{} {}", ym_label(app.month), app.month.0);
    let line = Line::from(vec![
        Span::styled("lgr", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("  ·  net worth ", Style::default().fg(MUTED)),
        Span::styled(
            format_minor(app.dashboard.net_worth),
            amount_style(app.dashboard.net_worth).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  ", Style::default().fg(MUTED)),
        Span::styled(month, Style::default().fg(ACCENT)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let index = match app.tab {
        Tab::Dashboard => 0,
        Tab::Accounts => 1,
        Tab::Register => 2,
        Tab::Reports => 3,
    };
    let tabs = Tabs::new([
        "1 Dashboard",
        "2 Accounts",
        "3 Register",
        "4 Reports",
    ])
    .select(index)
    .style(Style::default().fg(MUTED))
    .highlight_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
    .divider(" ");
    frame.render_widget(tabs, area);
}

fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App) {
    let [top, middle, bottom] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Percentage(55),
        Constraint::Min(6),
    ])
    .areas(area);

    draw_net_worth(frame, top, app);
    draw_net_chart(frame, middle, app);

    let [left, right] =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(bottom);
    draw_cash_flow(frame, left, app);
    draw_top_expenses(frame, right, app);
}

fn draw_net_worth(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered().title(" Net worth ");
    let delta = app.dashboard.net_worth_month;
    let sign = if delta < 0 { "" } else { "+" };
    let lines = vec![
        Line::from(Span::styled(
            format_minor(app.dashboard.net_worth),
            amount_style(app.dashboard.net_worth).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("{sign}{} this month", format_minor(delta)),
            amount_style(delta),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_net_chart(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered().title(" Net worth, 12 months ");
    let series = &app.dashboard.net_series;
    if series.len() < 2 || area.height < 5 {
        frame.render_widget(
            Paragraph::new(Span::styled("not enough data", Style::default().fg(MUTED))).block(block),
            area,
        );
        return;
    }

    let points: Vec<(f64, f64)> = series
        .iter()
        .enumerate()
        .map(|(i, (_, value))| (i as f64, *value as f64 / 100.0))
        .collect();
    let (lo, hi) = chart_bounds(&points);
    let last = series.len() - 1;
    let x_labels = vec![
        ym_label(report::ym_parse(&series[0].0)),
        ym_label(report::ym_parse(&series[last / 2].0)),
        ym_label(report::ym_parse(&series[last].0)),
    ];
    let y_labels = vec![format!("{lo:.0}"), format!("{hi:.0}")];

    let chart = Chart::new(vec![Dataset::default()
        .graph_type(GraphType::Line)
        .marker(Marker::Braille)
        .style(Style::default().fg(ACCENT))
        .data(&points)])
    .block(block)
    .x_axis(
        Axis::default()
            .bounds([0.0, last as f64])
            .style(Style::default().fg(MUTED))
            .labels(x_labels),
    )
    .y_axis(
        Axis::default()
            .bounds([lo, hi])
            .style(Style::default().fg(MUTED))
            .labels(y_labels),
    );

    frame.render_widget(chart, area);
}

/// Padded `[min, max]` for the net-worth axis; never a degenerate range and
/// keeps zero visible when the series crosses it.
fn chart_bounds(points: &[(f64, f64)]) -> (f64, f64) {
    let mut lo = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let mut hi = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    if lo == hi {
        return (lo - 1.0, hi + 1.0);
    }
    let pad = (hi - lo) * 0.05;
    let crosses_zero = lo < 0.0 && hi > 0.0;
    lo -= pad;
    hi += pad;
    if crosses_zero {
        lo = lo.min(0.0);
        hi = hi.max(0.0);
    }
    (lo, hi)
}

fn draw_cash_flow(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered().title(" Cash flow, 6 months ");
    if app.dashboard.flow.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("no transactions", Style::default().fg(MUTED))).block(block),
            area,
        );
        return;
    }

    let groups: Vec<BarGroup> = app
        .dashboard
        .flow
        .iter()
        .map(|(label, income, expense)| {
            BarGroup::default()
                .label(Line::from(label.clone()))
                .bars(&[
                    Bar::default()
                        .value((*income).max(0) as u64)
                        .style(Style::default().fg(OK)),
                    Bar::default()
                        .value((*expense).max(0) as u64)
                        .style(Style::default().fg(BAD)),
                ])
        })
        .collect();

    let mut chart = BarChart::default()
        .block(block)
        .bar_width(3)
        .bar_gap(0)
        .group_gap(2)
        .bar_style(Style::default().fg(OK))
        .value_style(Style::default().fg(MUTED))
        .label_style(Style::default().fg(MUTED));
    for group in groups {
        chart = chart.data(group);
    }
    frame.render_widget(chart, area);
}

fn draw_top_expenses(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered().title(" Top expenses ");
    if app.dashboard.top_expenses.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("no expenses", Style::default().fg(MUTED))).block(block),
            area,
        );
        return;
    }

    let max = app
        .dashboard
        .top_expenses
        .iter()
        .map(|(_, v)| *v)
        .max()
        .unwrap_or(1)
        .max(1);
    let lines: Vec<Line> = app
        .dashboard
        .top_expenses
        .iter()
        .map(|(label, value)| {
            let width = ((20 * value) / max).clamp(1, 20) as usize;
            Line::from(vec![
                Span::styled(format!("{label:<14.14}"), Style::default().fg(ACCENT)),
                Span::styled("█".repeat(width), Style::default().fg(BAD)),
                Span::raw(" "),
                Span::styled(
                    format!("{:>12}", format_minor(*value)),
                    Style::default().fg(WARN),
                ),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_accounts(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(["Account", "This month", "All time"])
        .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .accounts
        .iter()
        .map(|row| {
            let (label, style) = match row.header {
                Some(header) => (
                    header.to_string(),
                    Style::default().fg(WARN).add_modifier(Modifier::BOLD),
                ),
                None => {
                    let depth = row.name.split(':').count().saturating_sub(2);
                    (
                        format!("{}{}", "  ".repeat(depth), row.name),
                        Style::default(),
                    )
                }
            };
            Row::new(vec![
                Span::styled(label, style),
                Span::styled(
                    format!("{:>12}", format_minor(row.month)),
                    amount_style(row.month),
                ),
                Span::styled(
                    format!("{:>12}", format_minor(row.total)),
                    amount_style(row.total),
                ),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(40),
        Constraint::Length(14),
        Constraint::Length(14),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title(" Accounts "))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = TableState::default();
    if !app.accounts.is_empty() {
        state.select(Some(app.acct_sel.min(app.accounts.len() - 1)));
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_register(frame: &mut Frame, area: Rect, app: &App) {
    let Some(account) = app.reg_account.as_ref() else {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "pick an account in tab 2",
                Style::default().fg(MUTED),
            ))
            .block(Block::bordered().title(" Register ")),
            area,
        );
        return;
    };

    let balance = app
        .accounts
        .iter()
        .find(|row| row.header.is_none() && row.name == *account)
        .map(|row| row.total)
        .unwrap_or_else(|| app.register.iter().map(|row| row.delta).sum());
    let title = format!(" Register · {account} · balance {} ", format_minor(balance));

    let header = Row::new(["Date", "Payee", "Counter", "Amount", "Running"])
        .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = app
        .register
        .iter()
        .map(|row| {
            Row::new(vec![
                Span::styled(row.date.clone(), Style::default().fg(MUTED)),
                Span::raw(row.payee.clone()),
                Span::styled(row.counter.clone(), Style::default().fg(ACCENT)),
                Span::styled(
                    format!("{:>12}", format_minor(row.delta)),
                    amount_style(row.delta),
                ),
                Span::styled(
                    format!("{:>12}", format_minor(row.running)),
                    amount_style(row.running),
                ),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(10),
        Constraint::Min(20),
        Constraint::Percentage(40),
        Constraint::Length(14),
        Constraint::Length(14),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::bordered().title(title))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = TableState::default();
    if !app.register.is_empty() {
        state.select(Some(app.reg_sel.min(app.register.len() - 1)));
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_reports(frame: &mut Frame, area: Rect, app: &App) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

    let month = format!("{} {}", ym_label(app.month), app.month.0);
    let (assets, liabilities, equity) = app.reports.bs_totals;
    let bs_lines = report_lines(
        &app.reports.balance_sheet,
        &[
            ("Total assets", assets),
            ("Total liabilities", liabilities),
            ("Total equity", equity),
        ],
    );
    let (income, expenses) = app.reports.is_totals;
    let mut is_lines = report_lines(
        &app.reports.income_stmt,
        &[("Total income", income), ("Total expenses", expenses)],
    );
    is_lines.push(Line::from(""));
    is_lines.push(Line::from(vec![
        Span::styled(
            if app.reports.trial_ok { "trial balance ✓" } else { "trial balance ✗ (sum != 0)" },
            Style::default().fg(if app.reports.trial_ok { OK } else { BAD }),
        ),
        Span::raw("  "),
        Span::styled(format!("net income {}", format_minor(income - expenses)), {
            amount_style(income - expenses)
        }),
    ]));

    frame.render_widget(
        Paragraph::new(bs_lines).block(Block::bordered().title(format!(" Balance sheet · {month} "))),
        left,
    );
    frame.render_widget(
        Paragraph::new(is_lines)
            .block(Block::bordered().title(format!(" Income statement · {month} "))),
        right,
    );
}

fn report_lines(rows: &[(String, i64)], totals: &[(&str, i64)]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = rows
        .iter()
        .map(|(label, value)| {
            Line::from(vec![
                Span::styled(format!("{label:<22.22}"), Style::default().fg(ACCENT)),
                Span::styled(
                    format!("{:>16}", format_minor(*value)),
                    amount_style(*value),
                ),
            ])
        })
        .collect();
    lines.push(Line::from(""));
    for (label, value) in totals {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{label:<22}"),
                Style::default()
                    .fg(Color::Reset)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:>16}", format_minor(*value)),
                amount_style(*value).add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    lines
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let hints = match app.mode {
        Mode::Form => "Tab/↑↓ move  ·  Ctrl+T accept  ·  Ctrl+N add  ·  Ctrl+X drop  ·  Enter next  ·  Ctrl+S save  ·  Esc cancel",
        Mode::Help => "any key closes help",
        Mode::ConfirmDelete { .. } => "y confirm  ·  other key cancels",
        Mode::Browse => match app.tab {
            Tab::Dashboard => "1-4 tabs  ·  [ ] month  ·  n new  ·  r reload  ·  ? keys  ·  q quit",
            Tab::Accounts => "1-4 tabs  ·  j/k move  ·  Enter open register  ·  [ ] month  ·  n new  ·  ? keys  ·  q quit",
            Tab::Register => "1-4 tabs  ·  j/k move  ·  n new  ·  e edit  ·  d delete  ·  ? keys  ·  q quit",
            Tab::Reports => "1-4 tabs  ·  [ ] month  ·  r reload  ·  ? keys  ·  q quit",
        },
    };

    let mut spans = vec![Span::styled(hints, Style::default().fg(MUTED))];
    if let Mode::Form = app.mode {
        if let Some(form) = app.form.as_ref() {
            spans.push(Span::raw("  "));
            match form.posting_amounts() {
                Ok(amounts) => {
                    let sum: i64 = amounts.iter().sum();
                    spans.push(Span::styled(
                        format!("Σ = {}", format_minor(sum)),
                        if sum == 0 {
                            Style::default().fg(OK)
                        } else {
                            Style::default().fg(BAD)
                        },
                    ));
                    if sum != 0 {
                        spans.push(Span::styled(" unbalanced", Style::default().fg(BAD)));
                    }
                }
                Err(_) => spans.push(Span::styled("Σ = ?", Style::default().fg(BAD))),
            }
            if let Some(error) = &form.error {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(error.clone(), Style::default().fg(BAD)));
            }
        }
    }
    if let Some(status) = &app.status {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(status.clone(), Style::default().fg(WARN)));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let box_area = centered(area, 62, 18);
    let lines = vec![
        Line::from(Span::styled("Browse", Style::default().fg(ACCENT).bold())),
        Line::from("1-4            switch tab"),
        Line::from("j / k / ↑ / ↓  move selection"),
        Line::from("Enter          open register for the account"),
        Line::from("[ / ]          previous / next month"),
        Line::from("n              new transaction"),
        Line::from("e / d          edit / delete (register)"),
        Line::from("r              reload from disk"),
        Line::from("q              quit"),
        Line::from(""),
        Line::from(Span::styled("Form", Style::default().fg(ACCENT).bold())),
        Line::from("Tab / ↑ / ↓    move between fields"),
        Line::from("Ctrl+J / Ctrl+K cycle account suggestions"),
        Line::from("Ctrl+T         accept suggestion"),
        Line::from("Ctrl+N / Ctrl+X add / remove posting"),
        Line::from("Enter          next field (submit on last)"),
        Line::from("Ctrl+S         save    Esc cancel"),
    ];
    frame.render_widget(Clear, box_area);
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Keys ")),
        box_area,
    );
}

fn draw_confirm(frame: &mut Frame, area: Rect) {
    let box_area = centered(area, 32, 3);
    frame.render_widget(Clear, box_area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Delete transaction? y/N",
            Style::default().fg(WARN),
        )))
        .alignment(Alignment::Center)
        .block(Block::bordered().title(" Confirm ")),
        box_area,
    );
}

fn draw_form(frame: &mut Frame, area: Rect, form: &Form) {
    // 2 border rows + (2 + 2*n) field rows + 2 suggestion rows.
    let height = (6 + 2 * form.postings.len() as u16).min(area.height);
    let box_area = centered(area, 70, height);
    frame.render_widget(Clear, box_area);
    frame.render_widget(
        Paragraph::new("").block(Block::bordered().title(if form.editing.is_some() {
            " Edit transaction "
        } else {
            " New transaction "
        })),
        box_area,
    );

    let inner = Rect {
        x: box_area.x + 2,
        y: box_area.y + 1,
        width: box_area.width.saturating_sub(4),
        height: box_area.height.saturating_sub(2),
    };
    let rows = Layout::vertical(
        std::iter::repeat(Constraint::Length(1))
            .take(inner.height as usize)
            .collect::<Vec<_>>(),
    )
    .split(inner);

    let mut cursor: Option<(u16, u16)> = None;
    let mut line_index = 0usize;
    for index in 0..form.field_count() {
        if line_index >= rows.len() {
            break;
        }
        let focused = index == form.focus;
        let (label, value) = if index == 0 {
            ("Date".to_string(), form.date.clone())
        } else if index == 1 {
            ("Payee".to_string(), form.payee.clone())
        } else {
            let posting = (index - 2) / 2 + 1;
            if (index - 2) % 2 == 0 {
                (format!("Account {posting}"), form.postings[(index - 2) / 2].0.clone())
            } else {
                (format!("Amount {posting}"), form.postings[(index - 2) / 2].1.clone())
            }
        };
        let marker = if focused { ">" } else { " " };
        let style = if focused {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let line = Line::from(vec![
            Span::styled(format!("{marker} {label:<12} "), style),
            Span::raw(value.clone()),
        ]);
        frame.render_widget(Paragraph::new(line), rows[line_index]);
        if focused {
            let x = rows[line_index].x + 2 + 12 + 1 + value.chars().count() as u16;
            cursor = Some((x.min(rows[line_index].right().saturating_sub(1)), rows[line_index].y));
        }
        line_index += 1;

        if index == form.focus && form.account_field(index).is_some() {
            for suggestion in form.suggestions.iter().take(2) {
                if line_index >= rows.len() {
                    break;
                }
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!("    ↳ {suggestion}"),
                        Style::default().fg(WARN),
                    ))),
                    rows[line_index],
                );
                line_index += 1;
            }
        }
    }

    if let Some((x, y)) = cursor {
        frame.set_cursor_position((x, y));
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn rendered(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn seeded_app(month: (i32, u32)) -> App {
        let mut db = Db::open_memory().unwrap();
        db.insert_txn(
            "2026-09-15",
            "Lunch",
            &[("Expenses:Food".into(), 25_000), ("Assets:Bank:BCA".into(), -25_000)],
        )
        .unwrap();
        db.insert_txn(
            "2026-10-01",
            "Salary",
            &[("Assets:Bank:BCA".into(), 100_000), ("Income:Salary".into(), -100_000)],
        )
        .unwrap();
        db.insert_txn(
            "2026-11-01",
            "Salary",
            &[("Assets:Bank:BCA".into(), 100_000), ("Income:Salary".into(), -100_000)],
        )
        .unwrap();
        let mut app = App::new(&db).unwrap();
        app.month = month;
        app.reload(&db).unwrap();
        app
    }

    #[test]
    fn dashboard_renders_charts() {
        let app = seeded_app((2026, 11));
        let rendered = rendered(&app, 120, 40);
        assert!(rendered.contains("Net worth"), "header missing:\n{rendered}");
        assert!(rendered.contains("Cash flow"), "cash flow panel missing");
        assert!(rendered.contains("Top expenses"), "top expenses panel missing");
        assert!(rendered.contains("1,750.00") || rendered.contains("1,750"), "net worth missing");
    }

    #[test]
    fn dashboard_handles_single_point_series() {
        let mut db = Db::open_memory().unwrap();
        db.insert_txn(
            "2026-11-01",
            "Salary",
            &[("Assets:Bank:BCA".into(), 100_000), ("Income:Salary".into(), -100_000)],
        )
        .unwrap();
        let mut app = App::new(&db).unwrap();
        app.month = (2026, 11);
        app.reload(&db).unwrap();
        let rendered = rendered(&app, 120, 40);
        assert!(rendered.contains("not enough data"), "single point should degrade:\n{rendered}");
    }

    #[test]
    fn dashboard_handles_flat_series() {
        let app = seeded_app((2026, 11));
        // A zero-variance series would make the y axis degenerate; chart_bounds
        // must widen it instead of panicking.
        let flat = vec![(f64::from(0), 5.0), (f64::from(1), 5.0)];
        let (lo, hi) = chart_bounds(&flat);
        assert!(hi > lo, "bounds must not collapse: {lo}..{hi}");
        let _ = rendered(&app, 80, 24);
    }

    #[test]
    fn accounts_tab_lists_seeded_accounts() {
        let mut app = seeded_app((2026, 11));
        app.tab = Tab::Accounts;
        let rendered = rendered(&app, 120, 40);
        assert!(rendered.contains("ASSETS"), "assets group missing:\n{rendered}");
        assert!(rendered.contains("Assets:Bank:BCA"), "account row missing");
        assert!(rendered.contains("INCOME"), "income group missing");
    }

    #[test]
    fn reports_tab_renders_both_statements() {
        let mut app = seeded_app((2026, 11));
        app.tab = Tab::Reports;
        let rendered = rendered(&app, 120, 40);
        assert!(rendered.contains("Balance sheet"), "balance sheet missing:\n{rendered}");
        assert!(rendered.contains("Income statement"), "income statement missing");
        assert!(rendered.contains("trial balance"), "trial balance footer missing");
    }

    #[test]
    fn register_tab_renders_rows() {
        let mut app = seeded_app((2026, 11));
        app.reg_account = Some("Assets:Bank:BCA".to_string());
        app.tab = Tab::Register;
        app.reload(&Db::open_memory().unwrap()).ok();
        let rendered = rendered(&app, 120, 40);
        assert!(rendered.contains("Register"), "register panel missing:\n{rendered}");
    }

    #[test]
    fn form_overlay_renders_fields() {
        let mut app = seeded_app((2026, 11));
        app.mode = Mode::Form;
        app.form = Some(Form::new(vec!["Assets:Bank:BCA".to_string()]));
        let rendered = rendered(&app, 120, 40);
        assert!(rendered.contains("New transaction"), "form title missing:\n{rendered}");
        assert!(rendered.contains("Date"), "date field missing");
        assert!(rendered.contains("Account 1"), "account field missing");
    }
}
