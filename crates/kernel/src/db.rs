use rusqlite::types::{ToSqlOutput, Value as SqlValue, ValueRef};
use rusqlite::{Connection, ToSql, params_from_iter};
use serde::Deserialize;
use serde_json::{Map, Number, Value};
use uji_native::{Held, Json, List, native};

pub(crate) struct Db(Connection);

#[derive(Deserialize)]
struct Param(Value);

impl ToSql for Param {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match &self.0 {
            Value::Null => ToSqlOutput::Owned(SqlValue::Null),
            Value::Bool(flag) => ToSqlOutput::Owned(SqlValue::Integer(i64::from(*flag))),
            Value::Number(number) => ToSqlOutput::Owned(number.as_i64().map_or_else(
                || SqlValue::Real(number.as_f64().unwrap_or_default()),
                SqlValue::Integer,
            )),
            Value::String(text) => ToSqlOutput::Borrowed(ValueRef::Text(text.as_bytes())),
            Value::Array(_) | Value::Object(_) => {
                return Err(rusqlite::Error::ToSqlConversionFailure(
                    "a table cannot be stored in the database".into(),
                ));
            }
        })
    }
}

fn column(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(number) => Value::from(number),
        ValueRef::Real(number) => Number::from_f64(number).map_or(Value::Null, Value::Number),
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
            Value::String(String::from_utf8_lossy(bytes).into_owned())
        }
    }
}

#[native(db)]
fn open(path: &str) -> Result<Held<Db>, rusqlite::Error> {
    Connection::open(path).map(|connection| Held::new(Db(connection)))
}

#[native]
fn exec(db: &Db, sql: &str, params: Json<List<Param>>) -> Result<usize, rusqlite::Error> {
    let List(params) = params.0;
    if params.is_empty() {
        return db.0.execute_batch(sql).map(|()| 0);
    }
    db.0.prepare_cached(sql)?.execute(params_from_iter(params))
}

#[native]
fn query(
    db: &Db,
    sql: &str,
    params: Json<List<Param>>,
) -> Result<Json<Vec<Map<String, Value>>>, rusqlite::Error> {
    let mut statement = db.0.prepare_cached(sql)?;
    let columns: Vec<String> = statement
        .column_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut rows = statement.query(params_from_iter(params.0.0))?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut fields = Map::new();
        for (index, name) in columns.iter().enumerate() {
            fields.insert(name.clone(), column(row.get_ref(index)?));
        }
        out.push(fields);
    }
    Ok(Json(out))
}
