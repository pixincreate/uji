# Storage

## uji.db.open(path)

Opens or creates the SQLite database at `path`. The result is a database, or
`nil` and an error message. Parameters are a list that fills the `?` marks in
the statement, and `uji.db.null` stands for `NULL` in that list. A statement
that SQLite rejects raises an error, in every method below.

| Member | Meaning |
|---|---|
| `db:exec(sql, params)` | Runs a statement and gives the number of rows it changed. Without `params`, `sql` may hold several statements. |
| `db:query(sql, params)` | The rows, as a list of tables keyed by column name. |
| `db:transaction(fn)` | Runs `fn` in a transaction and gives back what it returns. An error inside `fn` rolls the transaction back and raises again. `fn` cannot wait, so no other task can write in the middle of the transaction. |
| `db:close()` | Closes the database. |
