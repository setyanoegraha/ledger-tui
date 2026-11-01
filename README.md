# ledger-tui

### Local double-entry bookkeeping in your terminal

**ledger-tui** is a TUI for tracking personal finances: no GUI, no network, no account.
One SQLite file, one binary. Written in Rust with [ratatui](https://ratatui.rs), Nord theme.

```text
lgr  ·  net worth 7,500.00  ·  Nov 2026
 1 Dashboard   2 Accounts   3 Register   4 Reports
┌ Net worth ───────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│7,500.00                                                                                                              │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌ Net worth, 12 months ────────────────────────────────────────────────────────────────────────────────────────────────┐
│7738│                                                                                                              ⣀⣀⠤│
│    │                                                                                                      ⢀⣀⡠⠤⠔⠒⠉⠉   │
│    │                                                                                               ⢀⣀⡠⠤⠒⠒⠉⠁          │
│    │                                                                                        ⣀⣀⠤⠤⠒⠊⠉⠁                 │
│    │                                                                                 ⣀⡠⠤⠔⠒⠊⠉                         │
│    │                                                                         ⢀⣀⡠⠤⠒⠒⠉⠉                                │
│    │                                                                  ⢀⣀⠤⠤⠒⠊⠉⠁                                       │
│    │                                                           ⣀⣀⠤⠔⠒⠊⠉⠁                                              │
│    │                                                   ⣀⣀⠤⠤⠒⠒⠉⠉                                                      │
│    │                                           ⣀⣀⠤⠤⠒⠒⠉⠉                                                              │
│    │                                  ⢀⣀⡠⠤⠔⠒⠊⠉⠉                                                                      │
│    │                          ⣀⣀⡠⠤⠔⠒⠊⠉⠁                                                                              │
│    │                  ⣀⣀⠤⠤⠒⠒⠉⠉                                                                                       │
│    │         ⢀⣀⡠⠤⠔⠒⠒⠉⠉                                                                                               │
│    │ ⢀⣀⡠⠤⠔⠒⠊⠉⠁                                                                                                       │
│2512│⠉⠁                                                                                                               │
│    └─────────────────────────────────────────────────────────────────────────────────────────────────────────────────│
│  Sep                                                      Oct                                                     Nov│
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
┌ Cash flow, 6 months ───────────────────────────────────────────┐┌ Top expenses ──────────────────────────────────────┐
│███     ███     ███                                             ││Food          ████████████████████       450.00     │
│███     ███     ███                                             ││                                                    │
│███     ███     ███                                             ││                                                    │
│███     ███     ███                                             ││                                                    │
│███     ███     ███                                             ││                                                    │
│███     ███     ███                                             ││                                                    │
│███     ███     ███                                             ││                                                    │
│███     ███     ███                                             ││                                                    │
│███     ███▇▇▇  ███                                             ││                                                    │
│███     ██████  ███▅▅▅                                          ││                                                    │
│███▇▇▇  ██████  ██████                                          ││                                                    │
│Sep     Oct     Nov                                             ││                                                    │
└────────────────────────────────────────────────────────────────┘└─────────────────────────────────────────────────────┘
1-4 tabs  ·  [ ] month  ·  n new  ·  r reload  ·  ? keys  ·  q quit  saved
```

---

## Features

* **Four tabs** — Dashboard, Accounts, Register, Reports. Every view is computed from the ledger on each reload.
* **Real double-entry** — five account kinds (Assets, Liabilities, Equity, Income, Expenses). Hierarchy comes from `:`-separated names, so `Assets:Bank:BCA` rolls up into `Assets:Bank` and `Assets` automatically.
* **Dashboard charts** — net worth line over the last 12 months (braille), income-vs-expense bars for the last 6 months, and the top 5 expense categories for the selected month.
* **Reports** — a cumulative balance sheet, a period income statement, and a trial-balance indicator that proves the ledger sums to zero.
* **Fast inline entry** — a form with account suggestions, multi-posting splits, and a live `Σ = 0.00` sign check before you save.
* **Exact money** — amounts are signed `i64` minor units (2 decimals). No floating point anywhere in storage or reports.

---

## Prerequisites

* A terminal with 256-color support (any modern terminal emulator). Charts are drawn with Unicode braille characters, so a font with braille coverage is required.
* Terminal width of at least ~48 columns; narrower windows clip the charts.
* Rust toolchain 1.89+ only if you build from source (`rusqlite` requires it; developed
  against rustc 1.98.1).

---

## Installation

```bash
git clone https://github.com/setyanoegraha/ledger-tui.git
cd ledger-tui
cargo install --path .
```

Or straight from git:

```bash
cargo install --git https://github.com/setyanoegraha/ledger-tui.git
```

Both install a binary named **`lgr`** (the binary name comes from `[[bin]]`, not from the
crate name, so the clone directory `ledger-tui` and the command `lgr` differ).

---

## Quick start

```bash
lgr
```

Then record your first transaction:

1. Press `n` — the transaction form opens with today's date and the cursor on `Payee`.
2. Type the description, `Tab`, then the two postings: `Assets:Bank:Checking` `1000` and
   `Income:Salary` `-1000`. The footer turns green with `Σ = 0.00`.
3. Press `Ctrl+S` to save. The dashboard header immediately shows the new net worth.

---

## Keys

### Browsing

| Key | Action |
| :--- | :--- |
| `1` `2` `3` `4` | Switch tab: Dashboard, Accounts, Register, Reports |
| `j` `k` `↑` `↓` | Move the selection |
| `Enter` | Open the register of the selected account (Accounts tab) |
| `[` `]` | Previous / next month |
| `n` | New transaction |
| `e` | Edit the selected transaction (Register tab) |
| `d` | Delete the selected transaction (Register tab) |
| `r` | Reload from disk |
| `?` | Show all keys |
| `q` | Quit |

### In the transaction form

| Key | Action |
| :--- | :--- |
| `Tab` / `↓` | Next field |
| `BackTab` / `↑` | Previous field |
| `Ctrl+J` / `Ctrl+K` | Cycle the account suggestions |
| `Ctrl+T` | Accept the highlighted suggestion |
| `Ctrl+N` | Add another posting |
| `Ctrl+X` | Remove the last posting (never below two) |
| `Enter` | Next field; on the last field, submit |
| `Ctrl+S` | Save from any field |
| `Esc` | Cancel and discard |

---

## Where your data lives

```
~/.lgr/ledger.db
```

A single SQLite database, created on first run. Override the location with the `LGR_DB`
environment variable:

```bash
LGR_DB=/path/to/my-books.db lgr
```

`lgr --help` prints the resolved path. Backup is `cp ~/.lgr/ledger.db ~/backups/ledger-$(date +%F).db`.

---

## Account names

The first segment of an account name decides its kind, case-insensitively:

| Root | Aliases accepted |
| :--- | :--- |
| `Assets` | `asset` |
| `Liabilities` | `liability` |
| `Equity` | — |
| `Income` | `revenue`, `revenues` |
| `Expenses` | `expense` |

Deeper segments are free-form: `Assets:Bank:BCA`, `Expenses:Food:Groceries`,
`Liabilities:Card:Visa`. Accounts are created the moment you first post to them; there is
no separate setup step. Anything whose root is not in this table is rejected with
`unknown account root: <name>`.

---

## Guide

New to double-entry, or unsure what to type for a credit-card payment?
**[guide.md](guide.md)** walks through the model, six worked examples, the monthly routine,
and every error message the app can show.

---

## Development

```bash
cargo test              # 40 unit tests
cargo build --release   # optimized binary at target/release/lgr
```

The code is split into `money` (parsing and formatting), `report` (pure accounting
helpers), `db` (SQLite), `app` (state and key handling) and `ui` (rendering), so the
accounting logic is testable without a terminal.

---

## License

MIT — see [LICENSE](LICENSE).
