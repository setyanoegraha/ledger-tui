# Guide — recording money with `lgr`

This guide assumes you have `lgr` installed and can open it. It covers just enough
accounting to make the right keystrokes, then six worked examples you can copy.

---

## The one idea

Every transaction has **at least two postings**, and their amounts must **sum to zero**.

`lgr` stores each posting as a signed amount where positive means *debit*. A balanced
transaction is therefore one whose postings add up to `0`. You never type the words
"debit" or "credit" — you type one positive amount and one negative amount, and the app
tells you whether they balance (`Σ = 0.00` in green at the bottom of the form).

Why bother? Because money never appears or disappears: it moves. Knowing both sides of a
move is what makes the reports trustworthy — the balance sheet, the income statement and
the trial balance all fall out of the same two numbers.

---

## The five account kinds

The first segment of an account name decides its kind. `lgr` accepts these roots
(case-insensitive), plus a few aliases:

| Kind | Increases with | Normal sign in `lgr` | Appears in |
| :--- | :--- | :---: | :--- |
| Assets | debit | `+` | Balance sheet |
| Liabilities | credit | `−` | Balance sheet |
| Equity | credit | `−` | Balance sheet |
| Income | credit | `−` | Income statement |
| Expenses | debit | `+` | Income statement |

Aliases: `asset`, `liability`, `revenue` / `revenues`, `expense`. Anything else is
rejected.

"Normal sign" means how the amount looks in the form. Spending `200` on a credit card
posts `+200` to `Expenses:Food` and `−200` to `Liabilities:Card`. The reports then display
the liability as a positive `200.00`, because they show each kind the way an accountant
reads it. You do not have to flip signs in your head — the rule is only "one side
positive, other side negative, total zero".

---

## Reading the four tabs

**Dashboard** — net worth (`Assets − Liabilities`), a 12-month net-worth line, income vs
expense bars for the last 6 months, and the top 5 expense categories for the month you are
viewing. `[` and `]` move the month; everything else follows.

**Accounts** — every account that has a balance, grouped by kind, with *This month* and
*All time* columns. Parent accounts roll up their children: `Assets:Bank:BCA` contributes
to `Assets:Bank` and to `Assets`. Select a row with `j`/`k` and press `Enter` to open its
register.

**Register** — the transactions of one account, newest first (up to 200 rows). *Counter*
shows the other accounts involved, *Amount* is the change to this account, *Running* is
the total inside the visible window. The panel title shows the account's real balance.
`n` opens a form prefilled with this account, `e` edits the selected row, `d` deletes it.

**Reports** — a cumulative balance sheet as of the end of the selected month, and an income
statement for that month only. `trial balance ✓` means the whole ledger sums to zero.

---

## Recording a transaction, step by step

Press `n` from any tab.

1. **Payee** — the form opens focused here. Type what it was, e.g. `Groceries`.
2. **Account 1** — `Tab`, then type the account. Suggestions appear inline; `Ctrl+T`
   accepts the highlighted one, `Ctrl+J`/`Ctrl+K` cycle between them.
3. **Amount 1** — `Tab`, then the amount. `250`, `1,250.50` and `-950` are all valid;
   `1,5` and `1.234` are not.
4. **Account 2**, **Amount 2** — the opposite side.
5. Check the footer: `Σ = 0.00` in green means balanced. Red with `unbalanced` means not.
6. `Ctrl+S` saves. `Esc` discards.

Extras:

* **Change the date** — press `↑` from `Payee` to reach `Date`, edit it as `YYYY-MM-DD`,
  then `↓` back. New transactions default to today.
* **More than two postings** — `Ctrl+N` adds a posting each time, `Ctrl+X` removes the
  last one (never below two, since a transaction needs both sides).
* **Edit instead of creating** — `e` in the Register reuses the same form and rewrites the
  transaction atomically; existing postings are replaced, not appended.

---

## Worked examples

All six use the current month so the dashboard and reports line up. Amounts carry no
currency symbol — `lgr` stores one currency with two decimals and does not care which.

### 1. Salary arrives

| Account | Amount |
| :--- | ---: |
| `Assets:Bank:BCA` | 3000 |
| `Income:Salary` | -3000 |

Cash increases, income is recognized. Net worth after: **3,000.00**.

### 2. Groceries paid from the bank account

| Account | Amount |
| :--- | ---: |
| `Expenses:Food` | 250 |
| `Assets:Bank:BCA` | -250 |

The expense grows and the asset shrinks by the same amount. Net worth after: **2,750.00**.

### 3. Groceries on a credit card

| Account | Amount |
| :--- | ---: |
| `Expenses:Food` | 200 |
| `Liabilities:Card` | -200 |

Nothing leaves the bank, yet net worth drops: the expense is real and the card balance is
a claim against you. Net worth after: **2,550.00**.

### 4. Paying the card

| Account | Amount |
| :--- | ---: |
| `Liabilities:Card` | 200 |
| `Assets:Bank:BCA` | -200 |

The debt shrinks and the bank account shrinks equally. Both are balance-sheet accounts, so
**net worth does not change**: **2,550.00**. Paying a debt is not an expense, and recording
it as one would double-count the spending from example 3.

### 5. Withdrawing cash

| Account | Amount |
| :--- | ---: |
| `Assets:Cash` | 100 |
| `Assets:Bank:BCA` | -100 |

Two assets trade places, so net worth stays **2,550.00**. The same pattern covers transfers
between any two asset accounts.

### 6. Rent and utilities in one transaction

| Account | Amount |
| :--- | ---: |
| `Expenses:Rent` | 800 |
| `Expenses:Utilities` | 150 |
| `Assets:Bank:BCA` | -950 |

One payment, two categories. Enter it with `Ctrl+N` for the third posting. Net worth after:
**1,600.00**.

After all six, the Accounts tab reads `Assets:Bank:BCA 1,500.00`,
`Assets:Cash 100.00`, `Liabilities:Card 0.00`, `Expenses:Food 450.00`,
`Expenses:Rent 800.00`, `Expenses:Utilities 150.00`, and Reports shows
`Total income 3,000.00`, `Total expenses 1,400.00`, `net income 1,600.00`,
`trial balance ✓`. Top expenses: `Rent 800.00`, `Food 450.00`, `Utilities 150.00`.

---

## Monthly routine

1. Record transactions as they happen; do not batch a month at the end.
2. At month end, open **Accounts** and read the *This month* column. Categories that look
   wrong usually mean a mis-typed account name.
3. Open **Reports**. `trial balance ✓` is your proof that nothing is corrupted.
4. Press `[` a few times to compare this month against previous ones on the Dashboard line.
5. Back up the file: `cp ~/.lgr/ledger.db ~/backups/ledger-$(date +%F).db`.

---

## Common mistakes

The footer (Form) and the status area show these messages verbatim:

| Message | Meaning | Fix |
| :--- | :--- | :--- |
| `postings must sum to zero` / `Σ = ... unbalanced` | The postings do not add to `0` | Find the posting with the wrong sign or amount |
| `payee is required` | The Payee field is empty | Type a description |
| `date must be YYYY-MM-DD` | The date is malformed | Use e.g. `2026-11-02` |
| `unknown account root: <name>` | The first segment is not one of the five kinds | Start with `Assets`, `Liabilities`, `Equity`, `Income` or `Expenses` |
| `amount N: invalid amount: <text>` | The amount at posting N is not a number | Plain digits, optional `,` thousands and up to two decimals |
| `amount N: empty amount` | The amount at posting N is blank | Fill both sides of the transaction |
| `need at least 2 postings` | Only reached from outside the UI | Add the second posting |
| `invalid account name: <name>` | Empty or doubled `:` such as `Assets:` or `a::b` | Remove the stray colon |

Accounts are created the moment you first post to them, and names are compared exactly.
`Assets:Bank:BCA` and `Assets:Bank:Bca` are two different accounts, and each will show its
own row in **Accounts**. If a category suddenly looks half-empty, check for a case or
spelling difference rather than assuming data was lost.

---

## What `lgr` deliberately does not do

No CSV import, no recurring transactions, no budgets, and no multi-currency. It keeps one
currency with two decimals and expects you to enter each transaction by hand. Nobody
claimed otherwise, and leaving those out is what keeps the ledger a single file you can
read with `sqlite3` and back up with `cp`.
