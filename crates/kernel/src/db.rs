use mlua::{Function, Lua, LuaSerdeExt, LuaString, MultiValue, Table, Value};
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, params_from_iter};
use uji_macros::{constant, function, methods};

#[derive(Debug, thiserror::Error)]
enum DbError {
    #[error("{0}")]
    Sql(#[from] rusqlite::Error),
    #[error("{0}")]
    Lua(#[from] mlua::Error),
    #[error("the database is closed")]
    Closed,
    #[error("a {0} cannot be stored in the database")]
    Unstorable(&'static str),
}

impl From<DbError> for mlua::Error {
    fn from(err: DbError) -> Self {
        Self::external(err)
    }
}

pub(crate) struct Db(Option<Connection>);

impl Db {
    fn connection(&self) -> Result<&Connection, DbError> {
        self.0.as_ref().ok_or(DbError::Closed)
    }

    fn batch(&self, sql: &str) -> Result<(), DbError> {
        Ok(self.connection()?.execute_batch(sql)?)
    }
}

fn sql_value(value: Value) -> Result<SqlValue, DbError> {
    Ok(match value {
        value if value.is_null() => SqlValue::Null,
        Value::Boolean(flag) => SqlValue::Integer(i64::from(flag)),
        Value::Integer(number) => SqlValue::Integer(number),
        Value::Number(number) => SqlValue::Real(number),
        Value::String(text) => match text.to_str() {
            Ok(text) => SqlValue::Text(text.to_string()),
            Err(_) => SqlValue::Blob(text.as_bytes().to_vec()),
        },
        other => return Err(DbError::Unstorable(other.type_name())),
    })
}

fn sql_values(params: Option<Vec<Value>>) -> Result<Vec<SqlValue>, DbError> {
    params
        .unwrap_or_default()
        .into_iter()
        .map(sql_value)
        .collect()
}

fn lua_value(lua: &Lua, value: ValueRef<'_>) -> Result<Value, DbError> {
    Ok(match value {
        ValueRef::Null => Value::Nil,
        ValueRef::Integer(number) => Value::Integer(number),
        ValueRef::Real(number) => Value::Number(number),
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => Value::String(lua.create_string(bytes)?),
    })
}

fn row(lua: &Lua, columns: &[LuaString], row: &rusqlite::Row<'_>) -> Result<Table, DbError> {
    let table = lua.create_table()?;
    for (index, name) in columns.iter().enumerate() {
        table.raw_set(name, lua_value(lua, row.get_ref(index)?)?)?;
    }
    Ok(table)
}

#[methods(raise)]
impl Db {
    fn exec(&self, sql: &str, params: Option<Vec<Value>>) -> Result<usize, DbError> {
        let params = sql_values(params)?;
        if params.is_empty() {
            return self.batch(sql).map(|()| 0);
        }
        let mut statement = self.connection()?.prepare_cached(sql)?;
        Ok(statement.execute(params_from_iter(params))?)
    }

    fn query(&self, lua: &Lua, sql: &str, params: Option<Vec<Value>>) -> Result<Table, DbError> {
        let mut statement = self.connection()?.prepare_cached(sql)?;
        let columns = statement
            .column_names()
            .into_iter()
            .map(|name| lua.create_string(name))
            .collect::<mlua::Result<Vec<_>>>()?;
        let mut rows = statement.query(params_from_iter(sql_values(params)?))?;
        let out = lua.create_table()?;
        while let Some(found) = rows.next()? {
            out.raw_push(row(lua, &columns, found)?)?;
        }
        Ok(out)
    }

    #[script("transaction.lua")]
    fn transaction(lua: &Lua, run: Function) -> mlua::Result<MultiValue> {
        lua.globals().get::<Function>("pcall")?.call(run)
    }

    fn close(&mut self) {
        self.0.take();
    }
}

#[function(db)]
fn open(path: &str) -> Result<Db, rusqlite::Error> {
    Connection::open(path).map(|connection| Db(Some(connection)))
}

#[constant(db)]
fn null(lua: &Lua) -> Value {
    lua.null()
}
