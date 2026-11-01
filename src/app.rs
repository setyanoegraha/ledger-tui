//! Application state and key handling. No ratatui here so every transition is
//! unit-testable; `ui.rs` only reads this state.

use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::db::{Db, RegisterRow};
use crate::money::{format_minor, parse_amount};
use crate::report::{self, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Accounts,
    Register,
    Reports,
}

/// `header` marks a group label row; account rows have `header == None`.
pub struct AccountRow {
    pub header: Option<&'static str>,
    pub name: String,
    pub kind: Kind,
    pub month: i64,
    pub total: i64,
}

pub struct RegRow {
    pub txn_id: i64,
    pub date: String,
    pub payee: String,
    pub counter: String,
    pub delta: i64,
    pub running: i64,
}

#[derive(Default)]
pub struct Dashboard {
    pub net_worth: i64,
    pub net_worth_month: i64,
    pub net_series: Vec<(String, i64)>,
    pub flow: Vec<(String, i64, i64)>,
    pub top_expenses: Vec<(String, i64)>,
}

#[derive(Default)]
pub struct Reports {
    pub balance_sheet: Vec<(String, i64)>,
    pub income_stmt: Vec<(String, i64)>,
    pub bs_totals: (i64, i64, i64),
    pub is_totals: (i64, i64),
    pub trial_ok: bool,
}

pub struct Form {
    pub date: String,
    pub payee: String,
    /// `(account, amount text)` per posting.
    pub postings: Vec<(String, String)>,
    pub focus: usize,
    pub editing: Option<i64>,
    pub suggestions: Vec<String>,
    pub sugg_idx: usize,
    pub account_names: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Browse,
    Form,
    Help,
    ConfirmDelete { txn_id: i64 },
}

pub struct App {
    pub tab: Tab,
    pub month: (i32, u32),
    pub accounts: Vec<AccountRow>,
    pub acct_sel: usize,
    pub register: Vec<RegRow>,
    pub reg_sel: usize,
    pub reg_account: Option<String>,
    pub dashboard: Dashboard,
    pub reports: Reports,
    pub mode: Mode,
    /// Present exactly when `mode == Mode::Form`.
    pub form: Option<Form>,
    pub status: Option<String>,
    pub quit: bool,
}

const DATE_FORMAT: &str = "%Y-%m-%d";

fn today_string() -> String {
    chrono::Local::now().date_naive().format(DATE_FORMAT).to_string()
}

fn kind_root(kind: Kind) -> &'static str {
    match kind {
        Kind::Asset => "Assets",
        Kind::Liability => "Liabilities",
        Kind::Equity => "Equity",
        Kind::Income => "Income",
        Kind::Expense => "Expenses",
    }
}

impl Form {
    pub fn new(account_names: Vec<String>) -> Form {
        Form {
            date: today_string(),
            payee: String::new(),
            postings: vec![(String::new(), String::new()), (String::new(), String::new())],
            focus: 1,
            editing: None,
            suggestions: Vec::new(),
            sugg_idx: 0,
            account_names,
            error: None,
        }
    }

    pub fn with_account(account_names: Vec<String>, account: &str) -> Form {
        let mut form = Form::new(account_names);
        form.postings[0].0 = account.to_string();
        form
    }

    pub fn from_txn(
        account_names: Vec<String>,
        id: i64,
        date: &str,
        payee: &str,
        postings: Vec<(String, i64)>,
    ) -> Form {
        Form {
            date: date.to_string(),
            payee: payee.to_string(),
            postings: postings
                .into_iter()
                .map(|(account, amount)| (account, format_minor(amount)))
                .collect(),
            focus: 1,
            editing: Some(id),
            suggestions: Vec::new(),
            sugg_idx: 0,
            account_names,
            error: None,
        }
    }

    pub fn field_count(&self) -> usize {
        2 + self.postings.len() * 2
    }

    pub fn get(&self, index: usize) -> &str {
        match index {
            0 => &self.date,
            1 => &self.payee,
            i => {
                let pair = self.postings.get((i - 2) / 2);
                match pair {
                    Some((account, amount)) if (i - 2) % 2 == 0 => account,
                    Some((_, amount)) => amount,
                    None => "",
                }
            }
        }
    }

    pub fn set(&mut self, index: usize, value: String) {
        match index {
            0 => self.date = value,
            1 => self.payee = value,
            i => {
                if let Some(pair) = self.postings.get_mut((i - 2) / 2) {
                    if (i - 2) % 2 == 0 {
                        pair.0 = value;
                    } else {
                        pair.1 = value;
                    }
                }
            }
        }
        self.refresh_suggestions();
    }

    pub fn push_char(&mut self, index: usize, c: char) {
        let mut value = self.get(index).to_string();
        value.push(c);
        self.set(index, value);
    }

    pub fn pop_char(&mut self, index: usize) {
        let mut value = self.get(index).to_string();
        value.pop();
        self.set(index, value);
    }

    /// Account-field index (even field >= 2) or `None`.
    pub fn account_field(&self, index: usize) -> Option<usize> {
        if index >= 2 && (index - 2) % 2 == 0 {
            Some((index - 2) / 2)
        } else {
            None
        }
    }

    pub fn refresh_suggestions(&mut self) {
        self.suggestions.clear();
        self.sugg_idx = 0;
        let Some(_) = self.account_field(self.focus) else {
            return;
        };
        let typed = self.get(self.focus).to_lowercase();
        if typed.is_empty() {
            return;
        }
        for name in &self.account_names {
            if name.to_lowercase().contains(&typed) && name.to_lowercase() != typed {
                self.suggestions.push(name.clone());
                if self.suggestions.len() == 2 {
                    break;
                }
            }
        }
    }

    pub fn posting_amounts(&self) -> Result<Vec<i64>, String> {
        self.postings
            .iter()
            .enumerate()
            .map(|(i, (_, amount))| {
                parse_amount(amount).map_err(|e| format!("amount {}: {e}", i + 1))
            })
            .collect()
    }
}

impl App {
    pub fn new(db: &Db) -> Result<App> {
        let today = chrono::Local::now().date_naive();
        let mut app = App {
            tab: Tab::Dashboard,
            month: (today.year(), today.month()),
            accounts: Vec::new(),
            acct_sel: 0,
            register: Vec::new(),
            reg_sel: 0,
            reg_account: None,
            dashboard: Dashboard::default(),
            reports: Reports::default(),
            mode: Mode::Browse,
            form: None,
            status: None,
            quit: false,
        };
        app.reload(db)?;
        Ok(app)
    }

    pub fn reload(&mut self, db: &Db) -> Result<()> {
        let (begin, end) = report::ym_bounds(self.month);
        self.reload_accounts(db, &begin, &end)?;
        self.reload_dashboard(db, &begin, &end)?;
        self.reload_reports(db, &begin, &end)?;
        if self.reg_account.is_some() {
            self.reload_register(db)?;
        }
        Ok(())
    }

    fn reload_accounts(&mut self, db: &Db, begin: &str, end: &str) -> Result<()> {
        let balances = db.balances(end)?;
        let sums = db.period_sums(begin, end)?;
        let mut rows = Vec::new();

        for kind in report::KIND_ORDER {
            let balance_rows: Vec<(String, i64)> = balances
                .iter()
                .filter(|(name, _)| report::kind_of(name) == Some(kind))
                .cloned()
                .collect();
            if balance_rows.is_empty() {
                continue;
            }
            let sum_rows: Vec<(String, i64)> = sums
                .iter()
                .filter(|(name, _)| report::kind_of(name) == Some(kind))
                .cloned()
                .collect();

            let group_total = report::display_amount(
                kind,
                balance_rows.iter().map(|(_, v)| v).sum::<i64>(),
            );
            let group_month =
                report::display_amount(kind, sum_rows.iter().map(|(_, v)| v).sum::<i64>());

            use std::collections::BTreeMap;
            let mut folded: BTreeMap<String, (i64, i64)> = BTreeMap::new();
            for (name, amount) in report::fold_depth(&balance_rows, 3) {
                folded.entry(name).or_insert((0, 0)).1 += amount;
            }
            for (name, amount) in report::fold_depth(&sum_rows, 3) {
                folded.entry(name).or_insert((0, 0)).0 += amount;
            }

            rows.push(AccountRow {
                header: Some(report::kind_label(kind)),
                name: kind_root(kind).to_string(),
                kind,
                month: group_month,
                total: group_total,
            });

            let mut accounts: Vec<AccountRow> = folded
                .into_iter()
                .map(|(name, (month, total))| {
                    let label = if name.split(':').count() == 1 {
                        "(root)".to_string()
                    } else {
                        name
                    };
                    AccountRow {
                        header: None,
                        name: label,
                        kind,
                        month: report::display_amount(kind, month),
                        total: report::display_amount(kind, total),
                    }
                })
                .collect();
            accounts.sort_by(|a, b| a.name.cmp(&b.name));
            rows.extend(accounts);
        }

        self.accounts = rows;
        if self.acct_sel >= self.accounts.len() {
            self.acct_sel = self.accounts.len().saturating_sub(1);
        }
        if self.accounts.get(self.acct_sel).map(|r| r.header.is_some()).unwrap_or(false) {
            self.step_account_selection(true);
        }
        Ok(())
    }

    fn reload_dashboard(&mut self, db: &Db, begin: &str, end: &str) -> Result<()> {
        let mut series: Vec<(String, i64)> = db
            .net_worth_by_month()?
            .into_iter()
            .filter(|(ym, _)| report::ym_parse(ym) <= self.month)
            .collect();
        if series.len() > 12 {
            series.drain(..series.len() - 12);
        }
        let net_worth = series.last().map(|(_, v)| *v).unwrap_or(0);
        let net_worth_month = if series.len() >= 2 {
            series[series.len() - 1].1 - series[series.len() - 2].1
        } else {
            0
        };

        let flow_begin = report::ym_bounds(report::ym_shift(self.month, -5)).0;
        let mut flow: Vec<(String, i64, i64)> = db
            .by_month(&flow_begin, end)?
            .into_iter()
            .map(|(ym, income, expense)| {
                (report::ym_label(report::ym_parse(&ym)), income, expense)
            })
            .collect();
        if flow.len() > 6 {
            flow.drain(..flow.len() - 6);
        }

        let expense_rows: Vec<(String, i64)> = db
            .period_sums(begin, end)?
            .into_iter()
            .filter(|(name, _)| report::kind_of(name) == Some(Kind::Expense))
            .collect();
        let mut top_expenses: Vec<(String, i64)> = report::fold_depth(&expense_rows, 2)
            .into_iter()
            .map(|(name, amount)| {
                (
                    report::abbrev(std::slice::from_ref(&name)),
                    report::display_amount(Kind::Expense, amount),
                )
            })
            .filter(|(_, amount)| *amount > 0)
            .collect();
        top_expenses.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        top_expenses.truncate(5);

        self.dashboard = Dashboard {
            net_worth,
            net_worth_month,
            net_series: series,
            flow,
            top_expenses,
        };
        Ok(())
    }

    fn reload_reports(&mut self, db: &Db, begin: &str, end: &str) -> Result<()> {
        let balances = db.balances(end)?;
        let sums = db.period_sums(begin, end)?;

        let mut balance_sheet = Vec::new();
        let mut bs_totals = (0i64, 0i64, 0i64);
        for kind in [Kind::Asset, Kind::Liability, Kind::Equity] {
            let rows: Vec<(String, i64)> = balances
                .iter()
                .filter(|(name, _)| report::kind_of(name) == Some(kind))
                .cloned()
                .collect();
            let total = report::display_amount(kind, rows.iter().map(|(_, v)| v).sum::<i64>());
            match kind {
                Kind::Asset => bs_totals.0 = total,
                Kind::Liability => bs_totals.1 = total,
                _ => bs_totals.2 = total,
            }
            balance_sheet.extend(report_rows(&rows, kind));
        }

        let mut income_stmt = Vec::new();
        let mut is_totals = (0i64, 0i64);
        for kind in [Kind::Income, Kind::Expense] {
            let rows: Vec<(String, i64)> = sums
                .iter()
                .filter(|(name, _)| report::kind_of(name) == Some(kind))
                .cloned()
                .collect();
            let total = report::display_amount(kind, rows.iter().map(|(_, v)| v).sum::<i64>());
            if kind == Kind::Income {
                is_totals.0 = total;
            } else {
                is_totals.1 = total;
            }
            income_stmt.extend(report_rows(&rows, kind));
        }

        self.reports = Reports {
            balance_sheet,
            income_stmt,
            bs_totals,
            is_totals,
            trial_ok: balances.iter().map(|(_, v)| v).sum::<i64>() == 0,
        };
        Ok(())
    }

    fn reload_register(&mut self, db: &Db) -> Result<()> {
        let account = self.reg_account.clone().unwrap_or_default();
        let rows: Vec<RegisterRow> = db.register(&account, 200)?;
        let ids: Vec<i64> = rows.iter().map(|r| r.txn_id).collect();
        let counters = db.counter_accounts(&ids, &account)?;

        let mut ascending: Vec<i64> = rows.iter().map(|r| r.delta).collect();
        ascending.reverse();
        let running = report::running_totals(&ascending);
        let mut running_desc = running;
        running_desc.reverse();

        self.register = rows
            .into_iter()
            .zip(running_desc)
            .map(|(row, running)| {
                let names: Vec<String> = counters
                    .iter()
                    .filter(|(txn_id, _)| *txn_id == row.txn_id)
                    .map(|(_, name)| name.clone())
                    .collect();
                RegRow {
                    txn_id: row.txn_id,
                    date: row.date,
                    payee: row.payee,
                    counter: if names.is_empty() {
                        "-".to_string()
                    } else {
                        report::abbrev(&names)
                    },
                    delta: row.delta,
                    running,
                }
            })
            .collect();
        if self.reg_sel >= self.register.len() {
            self.reg_sel = self.register.len().saturating_sub(1);
        }
        Ok(())
    }

    fn status_error(&mut self, error: anyhow::Error) {
        self.status = Some(format!("error: {error:#}"));
    }

    fn reload_or_report(&mut self, db: &Db) {
        if let Err(error) = self.reload(db) {
            self.status_error(error);
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, db: &mut Db) {
        match self.mode {
            Mode::Help => {
                if matches!(
                    key.code,
                    KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q')
                ) {
                    self.mode = Mode::Browse;
                }
            }
            Mode::ConfirmDelete { txn_id } => {
                if key.code == KeyCode::Char('y') {
                    if let Err(error) = db.delete_txn(txn_id) {
                        self.status_error(error);
                    } else {
                        self.status = Some("deleted".to_string());
                    }
                }
                self.mode = Mode::Browse;
                self.reload_or_report(db);
            }
            Mode::Form => self.handle_form_key(key, db),
            Mode::Browse => self.handle_browse_key(key, db),
        }
    }

    fn handle_browse_key(&mut self, key: KeyEvent, db: &mut Db) {
        match key.code {
            KeyCode::Char('1') => self.tab = Tab::Dashboard,
            KeyCode::Char('2') => self.tab = Tab::Accounts,
            KeyCode::Char('3') => self.tab = Tab::Register,
            KeyCode::Char('4') => self.tab = Tab::Reports,
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('n') => match db.account_names() {
                Ok(names) => {
                    let form = match (&self.reg_account, self.tab) {
                        (Some(account), Tab::Register) => {
                            Form::with_account(names, account)
                        }
                        _ => Form::new(names),
                    };
                    self.open_form(form);
                }
                Err(error) => self.status_error(error),
            },
            KeyCode::Char('r') => self.reload_or_report(db),
            KeyCode::Char('[') => {
                self.month = report::ym_shift(self.month, -1);
                self.reload_or_report(db);
            }
            KeyCode::Char(']') => {
                self.month = report::ym_shift(self.month, 1);
                self.reload_or_report(db);
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(db, 1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(db, -1),
            KeyCode::Enter if self.tab == Tab::Accounts => {
                if let Some(row) = self.accounts.get(self.acct_sel) {
                    if row.header.is_none() {
                        let name = if row.name == "(root)" {
                            kind_root(row.kind).to_string()
                        } else {
                            row.name.clone()
                        };
                        self.reg_account = Some(name);
                        self.reg_sel = 0;
                        self.tab = Tab::Register;
                        self.reload_or_report(db);
                    }
                }
            }
            KeyCode::Char('e') if self.tab == Tab::Register => self.open_edit(db),
            KeyCode::Char('d') if self.tab == Tab::Register => {
                if let Some(row) = self.register.get(self.reg_sel) {
                    self.mode = Mode::ConfirmDelete { txn_id: row.txn_id };
                }
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, db: &mut Db, delta: isize) {
        match self.tab {
            Tab::Accounts => self.step_account_selection(delta > 0),
            Tab::Register => {
                if delta > 0 && self.reg_sel + 1 < self.register.len() {
                    self.reg_sel += 1;
                } else if delta < 0 && self.reg_sel > 0 {
                    self.reg_sel -= 1;
                }
            }
            _ => {}
        }
        let _ = db;
    }

    /// Move `acct_sel` to the next account row in `forward` direction,
    /// skipping group headers.
    fn step_account_selection(&mut self, forward: bool) {
        let len = self.accounts.len();
        if len == 0 {
            self.acct_sel = 0;
            return;
        }
        let mut index = self.acct_sel.min(len - 1);
        for _ in 0..len {
            if self.accounts[index].header.is_none() {
                self.acct_sel = index;
                return;
            }
            if forward {
                index = (index + 1) % len;
            } else {
                index = (index + len - 1) % len;
            }
        }
        self.acct_sel = 0;
    }

    fn open_form(&mut self, form: Form) {
        self.form = Some(form);
        self.mode = Mode::Form;
        self.status = None;
    }

    fn open_edit(&mut self, db: &mut Db) {
        let Some(row) = self.register.get(self.reg_sel) else {
            return;
        };
        let txn_id = row.txn_id;
        match (db.account_names(), db.txn_postings(txn_id)) {
            (Ok(names), Ok(postings)) => {
                let form = Form::from_txn(names, txn_id, &row.date, &row.payee, postings);
                self.open_form(form);
            }
            (Err(error), _) | (_, Err(error)) => self.status_error(error),
        }
    }

    fn handle_form_key(&mut self, key: KeyEvent, db: &mut Db) {
        let Some(mut form) = self.form.take() else {
            self.mode = Mode::Browse;
            return;
        };
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let mut submit = false;

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Browse;
                self.status = None;
                return;
            }
            KeyCode::Char('s') if ctrl => submit = true,
            KeyCode::Char('t') if ctrl => {
                if form.account_field(form.focus).is_some() {
                    if let Some(pick) = form.suggestions.get(form.sugg_idx).cloned() {
                        form.set(form.focus, pick);
                    }
                }
            }
            KeyCode::Char('j') if ctrl => cycle_suggestion(&mut form, true),
            KeyCode::Char('k') if ctrl => cycle_suggestion(&mut form, false),
            KeyCode::Char('n') if ctrl => {
                form.postings.push((String::new(), String::new()));
                form.focus = form.field_count() - 2;
                form.refresh_suggestions();
            }
            KeyCode::Char('x') if ctrl => {
                if form.postings.len() > 2 {
                    form.postings.pop();
                    form.focus = form.focus.min(form.field_count() - 1);
                    form.refresh_suggestions();
                }
            }
            KeyCode::Tab | KeyCode::Down => {
                form.focus = (form.focus + 1).min(form.field_count() - 1);
                form.refresh_suggestions();
            }
            KeyCode::BackTab | KeyCode::Up => {
                form.focus = form.focus.saturating_sub(1);
                form.refresh_suggestions();
            }
            KeyCode::Backspace => form.pop_char(form.focus),
            KeyCode::Enter => {
                if form.focus + 1 >= form.field_count() {
                    submit = true;
                } else {
                    form.focus += 1;
                    form.refresh_suggestions();
                }
            }
            KeyCode::Char(c) => form.push_char(form.focus, c),
            _ => {}
        }

        if submit {
            self.submit_form(db, form);
        } else {
            self.form = Some(form);
            self.mode = Mode::Form;
        }
    }

    fn submit_form(&mut self, db: &mut Db, mut form: Form) {
        let outcome = (|| -> Result<()> {
            NaiveDate::parse_from_str(&form.date, DATE_FORMAT)
                .map_err(|_| anyhow::anyhow!("date must be YYYY-MM-DD"))?;
            if form.payee.trim().is_empty() {
                return Err(anyhow::anyhow!("payee is required"));
            }
            let amounts = form.posting_amounts().map_err(anyhow::Error::msg)?;
            if amounts.iter().sum::<i64>() != 0 {
                return Err(anyhow::anyhow!("postings must sum to zero"));
            }
            let mut postings: Vec<(String, i64)> = Vec::with_capacity(amounts.len());
            for ((account, _), amount) in form.postings.iter().zip(amounts) {
                let account = account.trim().to_string();
                if report::kind_of(&account).is_none() {
                    return Err(anyhow::anyhow!("unknown account root: {account}"));
                }
                postings.push((account, amount));
            }
            match form.editing {
                Some(id) => db.update_txn(id, &form.date, &form.payee, &postings)?,
                None => {
                    db.insert_txn(&form.date, &form.payee, &postings)?;
                }
            }
            Ok(())
        })();

        match outcome {
            Ok(()) => {
                self.mode = Mode::Browse;
                self.status = Some("saved".to_string());
                self.reload_or_report(db);
            }
            Err(error) => {
                form.error = Some(format!("{error:#}"));
                self.form = Some(form);
                self.mode = Mode::Form;
            }
        }
    }
}

fn cycle_suggestion(form: &mut Form, forward: bool) {
    if form.account_field(form.focus).is_none() || form.suggestions.is_empty() {
        return;
    }
    let len = form.suggestions.len();
    form.sugg_idx = if forward {
        (form.sugg_idx + 1) % len
    } else {
        (form.sugg_idx + len - 1) % len
    };
}

/// `(label, displayed value)` rows for one account kind, folded to depth 2.
/// Kind-root postings are labelled `(root)`.
fn report_rows(rows: &[(String, i64)], kind: Kind) -> Vec<(String, i64)> {
    report::fold_depth(rows, 2)
        .into_iter()
        .map(|(name, amount)| {
            let label = if name.split(':').count() == 1 {
                "(root)".to_string()
            } else {
                report::abbrev(std::slice::from_ref(&name))
            };
            (label, report::display_amount(kind, amount))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn typed(app: &mut App, db: &mut Db, text: &str) {
        for c in text.chars() {
            app.handle_key(key(KeyCode::Char(c)), db);
        }
    }

    fn app_with_salary() -> (App, Db) {
        let mut db = Db::open_memory().unwrap();
        db.insert_txn(
            "2026-11-01",
            "Salary",
            &[("Assets:Bank:BCA".into(), 1000), ("Income:Salary".into(), -1000)],
        )
        .unwrap();
        let mut app = App::new(&db).unwrap();
        app.month = (2026, 11);
        app.reload(&db).unwrap();
        (app, db)
    }

    #[test]
    fn tabs_switch_with_number_keys() {
        let (mut app, mut db) = app_with_salary();
        app.handle_key(key(KeyCode::Char('2')), &mut db);
        assert_eq!(app.tab, Tab::Accounts);
        app.handle_key(key(KeyCode::Char('4')), &mut db);
        assert_eq!(app.tab, Tab::Reports);
    }

    #[test]
    fn brackets_shift_month_across_years() {
        let (mut app, mut db) = app_with_salary();
        app.month = (2026, 1);
        app.handle_key(key(KeyCode::Char('[')), &mut db);
        assert_eq!(app.month, (2025, 12));
        app.handle_key(key(KeyCode::Char(']')), &mut db);
        assert_eq!(app.month, (2026, 1));
    }

    #[test]
    fn accounts_enter_opens_register() {
        let (mut app, mut db) = app_with_salary();
        app.handle_key(key(KeyCode::Char('2')), &mut db);
        let index = app
            .accounts
            .iter()
            .position(|row| row.name == "Assets:Bank:BCA")
            .expect("account row");
        app.acct_sel = index;
        app.handle_key(key(KeyCode::Enter), &mut db);
        assert_eq!(app.reg_account.as_deref(), Some("Assets:Bank:BCA"));
        assert_eq!(app.tab, Tab::Register);
        assert_eq!(app.register.len(), 1);
        assert_eq!(app.register[0].running, 1000);
    }

    #[test]
    fn header_rows_are_skipped_by_selection() {
        let (mut app, mut db) = app_with_salary();
        app.handle_key(key(KeyCode::Char('2')), &mut db);
        assert!(app.accounts[0].header.is_some());
        for _ in 0..10 {
            app.handle_key(key(KeyCode::Char('j')), &mut db);
            assert!(app.accounts[app.acct_sel].header.is_none());
        }
    }

    #[test]
    fn form_submit_rejects_unbalanced() {
        let (mut app, mut db) = app_with_salary();
        app.handle_key(key(KeyCode::Char('n')), &mut db);
        assert_eq!(app.mode, Mode::Form);
        typed(&mut app, &mut db, "Lunch");
        app.handle_key(key(KeyCode::Tab), &mut db);
        typed(&mut app, &mut db, "Expenses:Food");
        app.handle_key(key(KeyCode::Tab), &mut db);
        typed(&mut app, &mut db, "250");
        app.handle_key(key(KeyCode::Tab), &mut db);
        typed(&mut app, &mut db, "Assets:Bank:BCA");
        app.handle_key(key(KeyCode::Tab), &mut db);
        typed(&mut app, &mut db, "-100");
        app.handle_key(ctrl('s'), &mut db);

        assert_eq!(app.mode, Mode::Form);
        assert!(app.form.as_ref().unwrap().error.is_some());
    }

    #[test]
    fn form_submit_writes_and_closes() {
        let (mut app, mut db) = app_with_salary();
        app.handle_key(key(KeyCode::Char('n')), &mut db);
        typed(&mut app, &mut db, "Lunch");
        app.handle_key(key(KeyCode::Tab), &mut db);
        typed(&mut app, &mut db, "Expenses:Food");
        app.handle_key(key(KeyCode::Tab), &mut db);
        typed(&mut app, &mut db, "250");
        app.handle_key(key(KeyCode::Tab), &mut db);
        typed(&mut app, &mut db, "Assets:Bank:BCA");
        app.handle_key(key(KeyCode::Tab), &mut db);
        typed(&mut app, &mut db, "-250");
        app.handle_key(ctrl('s'), &mut db);

        assert_eq!(app.mode, Mode::Browse);
        assert_eq!(app.status.as_deref(), Some("saved"));
        assert!(app.form.is_none());
        let balances = db.balances("2026-12-01").unwrap();
        let food = balances
            .iter()
            .find(|(name, _)| name == "Expenses:Food")
            .map(|(_, v)| *v);
        assert_eq!(food, Some(25_000));
    }

    #[test]
    fn delete_confirmation_removes_transaction() {
        let (mut app, mut db) = app_with_salary();
        app.handle_key(key(KeyCode::Char('3')), &mut db);
        app.reg_account = Some("Assets:Bank:BCA".to_string());
        app.reload(&db).unwrap();
        assert_eq!(app.register.len(), 1);
        app.handle_key(key(KeyCode::Char('d')), &mut db);
        assert!(matches!(app.mode, Mode::ConfirmDelete { .. }));
        app.handle_key(key(KeyCode::Char('y')), &mut db);
        assert_eq!(app.mode, Mode::Browse);
        assert!(app.register.is_empty());
    }

    #[test]
    fn register_new_form_prefills_the_open_account() {
        let (mut app, mut db) = app_with_salary();
        app.reg_account = Some("Assets:Bank:BCA".to_string());
        app.tab = Tab::Register;
        app.reload(&db).unwrap();
        app.handle_key(key(KeyCode::Char('n')), &mut db);
        let form = app.form.as_ref().expect("form open");
        assert_eq!(form.postings[0].0, "Assets:Bank:BCA");
    }

    #[test]
    fn ctrl_t_accepts_suggestion_but_tab_does_not() {
        let (mut app, mut db) = app_with_salary();
        app.handle_key(key(KeyCode::Char('n')), &mut db);
        app.handle_key(key(KeyCode::Tab), &mut db); // account field 1
        typed(&mut app, &mut db, "Assets:Bank");
        let form = app.form.as_ref().unwrap();
        assert_eq!(form.suggestions.len(), 1);
        let focus = form.focus;

        app.handle_key(key(KeyCode::Tab), &mut db);
        assert_eq!(app.form.as_ref().unwrap().get(focus), "Assets:Bank");

        app.handle_key(key(KeyCode::BackTab), &mut db);
        app.handle_key(ctrl('t'), &mut db);
        assert_eq!(app.form.as_ref().unwrap().get(focus), "Assets:Bank:BCA");
    }

    #[test]
    fn ctrl_x_keeps_minimum_two_postings() {
        let (mut app, mut db) = app_with_salary();
        app.handle_key(key(KeyCode::Char('n')), &mut db);
        let before = app.form.as_ref().unwrap().postings.len();
        app.handle_key(ctrl('x'), &mut db);
        assert_eq!(app.form.as_ref().unwrap().postings.len(), before);
        app.handle_key(ctrl('n'), &mut db);
        assert_eq!(app.form.as_ref().unwrap().postings.len(), 3);
    }

    #[test]
    fn reports_balance_sheet_and_income_statement() {
        let (app, _db) = app_with_salary();
        assert_eq!(app.reports.bs_totals.0, 1000); // assets
        assert_eq!(app.reports.bs_totals.1, 0); // liabilities
        assert_eq!(app.reports.is_totals.0, 1000); // income
        assert_eq!(app.reports.is_totals.1, 0); // expenses
        assert!(app.reports.trial_ok);
    }

    #[test]
    fn dashboard_series_and_net_worth() {
        let (app, _db) = app_with_salary();
        assert_eq!(app.dashboard.net_worth, 1000);
        assert_eq!(app.dashboard.net_series.len(), 1);
        assert_eq!(app.dashboard.flow.len(), 1);
        assert_eq!(app.dashboard.flow[0].1, 1000);
    }
}
