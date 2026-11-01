//! SQLite ledger. Signed postings, debit positive; no debit/credit columns.
//! Account hierarchy is derived from the `:`-separated name, not parent ids.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection};

use crate::report;

pub struct Db {
    conn: Connection,
}

pub struct RegisterRow {
    pub txn_id: i64,
    pub date: String,
    pub payee: String,
    pub delta: i64,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS account (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS txn (
  id    INTEGER PRIMARY KEY,
  date  TEXT NOT NULL,
  payee TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS posting (
  id         INTEGER PRIMARY KEY,
  txn_id     INTEGER NOT NULL REFERENCES txn(id) ON DELETE CASCADE,
  account_id INTEGER NOT NULL REFERENCES account(id),
  amount     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS txn_date ON txn(date);
CREATE INDEX IF NOT EXISTS posting_txn ON posting(txn_id);
CREATE INDEX IF NOT EXISTS posting_account ON posting(account_id);
INSERT OR IGNORE INTO account(name) VALUES
  ('Assets'), ('Liabilities'), ('Equity'), ('Income'), ('Expenses');
";

impl Db {
    /// Opens (creating parent dirs) at `path`, then migrates.
    pub fn open(path: &Path) -> Result<Db> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        Self::from_conn(conn)
    }

    /// In-memory database for tests.
    #[cfg(test)]
    pub fn open_memory() -> Result<Db> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> Result<Db> {
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    pub fn account_names(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT name FROM account ORDER BY name")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn insert_txn(
        &mut self,
        date: &str,
        payee: &str,
        postings: &[(String, i64)],
    ) -> Result<i64> {
        validate_postings(postings)?;
        let tx = self.conn.transaction()?;
        let id = insert_rows(&tx, None, date, payee, postings)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn update_txn(
        &mut self,
        id: i64,
        date: &str,
        payee: &str,
        postings: &[(String, i64)],
    ) -> Result<()> {
        validate_postings(postings)?;
        let tx = self.conn.transaction()?;
        insert_rows(&tx, Some(id), date, payee, postings)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_txn(&mut self, id: i64) -> Result<()> {
        // Cascade removes postings only when foreign_keys is on (set in from_conn).
        self.conn.execute("DELETE FROM txn WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn txn_postings(&self, id: i64) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.name, p.amount FROM posting p JOIN account a ON a.id = p.account_id
              WHERE p.txn_id = ?1 ORDER BY p.id",
        )?;
        let rows = stmt.query_map(params![id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Cumulative balance per account name for transactions before `end`.
    pub fn balances(&self, end: &str) -> Result<Vec<(String, i64)>> {
        self.sums("t.date < ?1", params![end])
    }

    /// Movement per account name for `begin <= date < end`.
    pub fn period_sums(&self, begin: &str, end: &str) -> Result<Vec<(String, i64)>> {
        self.sums("t.date >= ?1 AND t.date < ?2", params![begin, end])
    }

    fn sums(&self, filter: &str, args: impl rusqlite::Params) -> Result<Vec<(String, i64)>> {
        let sql = format!(
            "SELECT a.name, SUM(p.amount) FROM posting p
               JOIN txn t ON t.id = p.txn_id JOIN account a ON a.id = p.account_id
              WHERE {filter} GROUP BY a.name"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(args, |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// `(ym, income_display, expense_display)` per month, ascending.
    pub fn by_month(&self, begin: &str, end: &str) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT substr(t.date,1,7) AS ym,
                    COALESCE(SUM(CASE WHEN a.name = 'Income' OR a.name LIKE 'Income:%'
                                      THEN -p.amount ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN a.name = 'Expenses' OR a.name LIKE 'Expenses:%'
                                      THEN p.amount ELSE 0 END), 0)
               FROM posting p JOIN txn t ON t.id = p.txn_id JOIN account a ON a.id = p.account_id
              WHERE t.date >= ?1 AND t.date < ?2
              GROUP BY ym ORDER BY ym",
        )?;
        let rows = stmt.query_map(params![begin, end], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Cumulative displayed net worth per month from the earliest transaction.
    /// Liabilities are stored negative, so assets minus liabilities is simply
    /// the raw sum of asset and liability postings.
    pub fn net_worth_by_month(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT substr(t.date,1,7) AS ym,
                    COALESCE(SUM(CASE WHEN a.name = 'Assets' OR a.name LIKE 'Assets:%'
                                      THEN p.amount
                                      WHEN a.name = 'Liabilities' OR a.name LIKE 'Liabilities:%'
                                      THEN p.amount
                                      ELSE 0 END), 0)
               FROM posting p JOIN txn t ON t.id = p.txn_id JOIN account a ON a.id = p.account_id
              GROUP BY ym ORDER BY ym",
        )?;
        let deltas: Vec<(String, i64)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut running = 0i64;
        Ok(deltas
            .into_iter()
            .map(|(ym, delta)| {
                running += delta;
                (ym, running)
            })
            .collect())
    }

    /// Newest first, at most `limit` rows.
    pub fn register(&self, account: &str, limit: usize) -> Result<Vec<RegisterRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.date, t.payee, SUM(p.amount) AS delta
               FROM txn t JOIN posting p ON p.txn_id = t.id JOIN account a ON a.id = p.account_id
              WHERE (a.name = ?1 OR a.name LIKE ?1 || ':%')
              GROUP BY t.id ORDER BY t.date DESC, t.id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![account, limit as i64], |row| {
            Ok(RegisterRow {
                txn_id: row.get(0)?,
                date: row.get(1)?,
                payee: row.get(2)?,
                delta: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// `(txn_id, other_account_name)` for the given transactions.
    pub fn counter_accounts(&self, ids: &[i64], account: &str) -> Result<Vec<(i64, String)>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT p.txn_id, a.name FROM posting p JOIN account a ON a.id = p.account_id
              WHERE p.txn_id IN ({placeholders})
                AND NOT (a.name = ?{acct} OR a.name LIKE ?{acct} || ':%')
              ORDER BY p.txn_id, a.name",
            acct = ids.len() + 1
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let mut values: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(ids.len() + 1);
        for id in ids {
            values.push(id);
        }
        values.push(&account);
        let rows = stmt.query_map(values.as_slice(), |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

fn validate_postings(postings: &[(String, i64)]) -> Result<()> {
    if postings.len() < 2 {
        return Err(anyhow!("need at least 2 postings"));
    }
    let mut total = 0i64;
    for (_, amount) in postings {
        total = total
            .checked_add(*amount)
            .ok_or_else(|| anyhow!("posting total overflow"))?;
    }
    if total != 0 {
        return Err(anyhow!("postings must sum to zero"));
    }
    for (name, _) in postings {
        if report::kind_of(name).is_none() {
            return Err(anyhow!("unknown account root: {name}"));
        }
    }
    Ok(())
}

fn insert_rows(
    tx: &rusqlite::Transaction<'_>,
    id: Option<i64>,
    date: &str,
    payee: &str,
    postings: &[(String, i64)],
) -> Result<i64> {
    let txn_id = match id {
        Some(existing) => {
            tx.execute(
                "UPDATE txn SET date = ?1, payee = ?2 WHERE id = ?3",
                params![date, payee, existing],
            )?;
            tx.execute("DELETE FROM posting WHERE txn_id = ?1", params![existing])?;
            existing
        }
        None => {
            tx.execute(
                "INSERT INTO txn (date, payee) VALUES (?1, ?2)",
                params![date, payee],
            )?;
            tx.last_insert_rowid()
        }
    };

    for (name, amount) in postings {
        let account_id = ensure_account(tx, name)?;
        tx.execute(
            "INSERT INTO posting (txn_id, account_id, amount) VALUES (?1, ?2, ?3)",
            params![txn_id, account_id, amount],
        )?;
    }
    Ok(txn_id)
}

fn ensure_account(tx: &rusqlite::Transaction<'_>, name: &str) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() || name.split(':').any(|part| part.trim().is_empty()) {
        return Err(anyhow!("invalid account name: {name}"));
    }
    tx.execute("INSERT OR IGNORE INTO account (name) VALUES (?1)", params![name])?;
    let id = tx.query_row(
        "SELECT id FROM account WHERE name = ?1",
        params![name],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txn(db: &mut Db, date: &str, payee: &str, rows: &[(&str, i64)]) {
        let postings: Vec<(String, i64)> =
            rows.iter().map(|(n, a)| (n.to_string(), *a)).collect();
        db.insert_txn(date, payee, &postings).unwrap();
    }

    #[test]
    fn migrate_is_idempotent() {
        let db = Db::open_memory().unwrap();
        db.migrate().unwrap();
        assert_eq!(db.account_names().unwrap().len(), 5);
    }

    #[test]
    fn rejects_invalid_postings() {
        let mut db = Db::open_memory().unwrap();
        assert!(db
            .insert_txn("2026-11-01", "x", &[("Assets:Cash".into(), 100)])
            .is_err());
        assert!(db
            .insert_txn(
                "2026-11-01",
                "x",
                &[("Assets:Cash".into(), 100), ("Income:Salary".into(), -50)]
            )
            .is_err());
        assert!(db
            .insert_txn(
                "2026-11-01",
                "x",
                &[("Foo:Bar".into(), 100), ("Income:Salary".into(), -100)]
            )
            .is_err());
        assert!(db
            .insert_txn(
                "2026-11-01",
                "x",
                &[("Assets:".into(), 100), ("Income:Salary".into(), -100)]
            )
            .is_err());
    }

    #[test]
    fn creates_accounts_and_reports_balances() {
        let mut db = Db::open_memory().unwrap();
        txn(&mut db, "2026-11-01", "Salary", &[("Assets:Bank:BCA", 1000), ("Income:Salary", -1000)]);
        txn(&mut db, "2026-11-02", "Lunch", &[("Expenses:Food", 250), ("Assets:Bank:BCA", -250)]);

        let names = db.account_names().unwrap();
        assert!(names.contains(&"Assets:Bank:BCA".to_string()));
        assert!(names.contains(&"Income:Salary".to_string()));

        let balances = db.balances("2026-12-01").unwrap();
        let get = |name: &str| {
            balances
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| *v)
                .unwrap()
        };
        assert_eq!(get("Assets:Bank:BCA"), 750);
        assert_eq!(get("Income:Salary"), -1000);
        assert_eq!(get("Expenses:Food"), 250);
        assert_eq!(balances.iter().map(|(_, v)| v).sum::<i64>(), 0);
    }

    #[test]
    fn period_sums_respect_the_window() {
        let mut db = Db::open_memory().unwrap();
        txn(&mut db, "2026-09-15", "Lunch", &[("Expenses:Food", 250), ("Assets:Bank:BCA", -250)]);
        txn(&mut db, "2026-11-02", "Dinner", &[("Expenses:Food", 100), ("Assets:Bank:BCA", -100)]);

        let rows = db.period_sums("2026-11-01", "2026-12-01").unwrap();
        assert_eq!(rows, vec![("Assets:Bank:BCA".to_string(), -100), ("Expenses:Food".to_string(), 100)]);
    }

    #[test]
    fn register_includes_subaccounts_for_parent() {
        let mut db = Db::open_memory().unwrap();
        txn(&mut db, "2026-11-01", "Salary", &[("Assets:Bank:BCA", 1000), ("Income:Salary", -1000)]);
        txn(&mut db, "2026-11-02", "Move", &[("Assets:Bank:Mandiri", 400), ("Assets:Bank:BCA", -400)]);

        let rows = db.register("Assets", 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].date, "2026-11-02"); // newest first
        assert_eq!(rows[1].payee, "Salary");

        let counters = db.counter_accounts(&[1], "Assets").unwrap();
        assert_eq!(counters, vec![(1, "Income:Salary".to_string())]);
        assert!(db.counter_accounts(&[], "Assets").unwrap().is_empty());
    }

    #[test]
    fn delete_cascades_to_postings() {
        let mut db = Db::open_memory().unwrap();
        let id = db
            .insert_txn(
                "2026-11-01",
                "Salary",
                &[("Assets:Bank:BCA".into(), 1000), ("Income:Salary".into(), -1000)],
            )
            .unwrap();
        db.delete_txn(id).unwrap();
        assert!(db.txn_postings(id).unwrap().is_empty());
        assert_eq!(db.balances("2027-01-01").unwrap(), Vec::new());
    }

    #[test]
    fn update_replaces_postings() {
        let mut db = Db::open_memory().unwrap();
        let id = db
            .insert_txn(
                "2026-11-01",
                "Salary",
                &[("Assets:Bank:BCA".into(), 1000), ("Income:Salary".into(), -1000)],
            )
            .unwrap();
        db.update_txn(
            id,
            "2026-11-05",
            "Raise",
            &[("Assets:Bank:BCA".into(), 1500), ("Income:Salary".into(), -1500)],
        )
        .unwrap();
        assert_eq!(
            db.txn_postings(id).unwrap(),
            vec![
                ("Assets:Bank:BCA".to_string(), 1500),
                ("Income:Salary".to_string(), -1500)
            ]
        );
        assert_eq!(db.balances("2026-12-01").unwrap()[0].1, 1500);
    }

    #[test]
    fn reports_net_worth_cumulatively() {
        let mut db = Db::open_memory().unwrap();
        txn(&mut db, "2026-09-01", "Loan", &[("Assets:Bank:BCA", 1000), ("Liabilities:Card", -1000)]);
        txn(&mut db, "2026-10-01", "Salary", &[("Assets:Bank:BCA", 1000), ("Income:Salary", -1000)]);

        let series = db.net_worth_by_month().unwrap();
        assert_eq!(series, vec![("2026-09".to_string(), 0), ("2026-10".to_string(), 1000)]);
    }

    #[test]
    fn monthly_flow_separates_income_and_expense() {
        let mut db = Db::open_memory().unwrap();
        txn(&mut db, "2026-10-01", "Salary", &[("Assets:Bank:BCA", 1000), ("Income:Salary", -1000)]);
        txn(&mut db, "2026-10-02", "Lunch", &[("Expenses:Food", 250), ("Assets:Bank:BCA", -250)]);
        txn(&mut db, "2026-11-02", "Dinner", &[("Expenses:Food", 100), ("Assets:Bank:BCA", -100)]);

        let flow = db.by_month("2026-10-01", "2026-12-01").unwrap();
        assert_eq!(
            flow,
            vec![
                ("2026-10".to_string(), 1000, 250),
                ("2026-11".to_string(), 0, 100),
            ]
        );
    }
}
