use rusqlite::types::{ToSqlOutput, Value as SqlValue, ValueRef};
use rusqlite::{Connection, ToSql, params_from_iter};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Number, Value};
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

struct Rows {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

struct Row<'a> {
    columns: &'a [String],
    values: &'a [Value],
}

impl Serialize for Row<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.columns.len()))?;
        for (name, value) in self.columns.iter().zip(self.values) {
            map.serialize_entry(name, value)?;
        }
        map.end()
    }
}

impl Serialize for Rows {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut rows = serializer.serialize_seq(Some(self.rows.len()))?;
        for values in &self.rows {
            rows.serialize_element(&Row {
                columns: &self.columns,
                values,
            })?;
        }
        rows.end()
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
fn query(db: &Db, sql: &str, params: Json<List<Param>>) -> Result<Json<Rows>, rusqlite::Error> {
    let mut statement = db.0.prepare_cached(sql)?;
    let columns: Vec<String> = statement
        .column_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let width = columns.len();
    let rows = statement
        .query_map(params_from_iter(params.0.0), |row| {
            (0..width)
                .map(|index| row.get_ref(index).map(column))
                .collect()
        })?
        .collect::<Result<_, _>>()?;
    Ok(Json(Rows { columns, rows }))
}
