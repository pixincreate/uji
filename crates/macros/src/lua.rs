use crate::ctypes::key;
use crate::native::Argument;

const KEYWORDS: [&str; 12] = [
    "and", "elseif", "end", "function", "goto", "local", "nil", "not", "or", "repeat", "then",
    "until",
];

pub(crate) enum Passing {
    Sized,
    MaybeSized,
    Encoded,
    Handle(String),
    Flag,
    Optional,
    Pointer,
    Plain,
}

pub(crate) enum Shape {
    Data,
    Number,
    Table,
    Done,
    Object(String),
    Values,
}

pub(crate) enum Output {
    Nothing,
    Scalar {
        c: String,
        wide: bool,
        pointer: bool,
    },
    Answer {
        soft: bool,
        shape: Shape,
    },
}

pub(crate) struct Native<'a> {
    pub(crate) name: &'a str,
    pub(crate) place: &'a str,
    pub(crate) arguments: &'a [Argument],
    pub(crate) output: &'a Output,
    pub(crate) asynchronous: bool,
    pub(crate) receiver: bool,
    pub(crate) iterate: Option<String>,
}

fn lua_name(argument: &Argument) -> String {
    let name = key(&argument.name);
    if KEYWORDS.contains(&name.as_str()) {
        format!("{name}_")
    } else {
        name
    }
}

fn shape(output: &Output) -> String {
    let Output::Answer { shape, .. } = output else {
        return "nil".to_string();
    };
    match shape {
        Shape::Data => "nil".to_string(),
        Shape::Number => "numeric".to_string(),
        Shape::Table => "decoded".to_string(),
        Shape::Done => "done".to_string(),
        Shape::Object(class) => format!("object({class:?})"),
        Shape::Values => "unpacked".to_string(),
    }
}

fn raw(native: &Native<'_>) -> bool {
    native
        .arguments
        .iter()
        .any(|argument| matches!(argument.passing, Passing::Pointer))
        || matches!(native.output, Output::Scalar { pointer: true, .. })
}

fn function(native: &Native<'_>) -> String {
    let mut parameters = Vec::new();
    if native.receiver {
        parameters.push("self".to_string());
    }
    let mut prepared = Vec::new();
    let mut values = Vec::new();
    for (index, argument) in native.arguments.iter().enumerate() {
        let name = lua_name(argument);
        match &argument.passing {
            Passing::Handle(_) if index == 0 => {
                parameters.push("self".to_string());
                values.push("self.handle".to_string());
            }
            Passing::Handle(_) => values.push(format!("{name}.handle")),
            Passing::Sized => values.push(format!("{name}, #{name}")),
            Passing::MaybeSized => values.push(format!("{name}, {name} and #{name} or 0")),
            Passing::Encoded => {
                prepared.push(format!(
                    "    local _{name} = _encode({name} == nil and _empty or {name})\n"
                ));
                values.push(format!("_{name}, #_{name}"));
            }
            Passing::Flag => values.push(format!("{name} == true")),
            Passing::Optional => values.push(format!("{name} or 0, {name} ~= nil")),
            Passing::Pointer | Passing::Plain => values.push(name.clone()),
        }
        if index > 0 || !matches!(argument.passing, Passing::Handle(_)) {
            parameters.push(name);
        }
    }
    let call = format!("_native({})", values.join(", "));
    let result = match native.output {
        Output::Nothing => call,
        Output::Scalar { wide: true, .. } => format!("return tonumber({call})"),
        Output::Scalar { .. } => format!("return {call}"),
        Output::Answer { .. } if native.asynchronous => {
            format!("return _finish(_wait({call}), _shape)")
        }
        Output::Answer { .. } => format!("return _finish({call}, _shape)"),
    };
    let finish = match native.output {
        Output::Answer { soft: true, .. } => "settle",
        _ => "value",
    };
    format!(
        "local _native, _encode, _empty = natives.{}, encode, EMPTY\nlocal _finish, _wait, _shape = {finish}, wait, {}\nreturn function({})\n{}    {result}\nend\n",
        native.name,
        shape(native.output),
        parameters.join(", "),
        prepared.concat(),
    )
}

fn iterator(method: &str) -> String {
    format!(
        "return function(self)\n    return function()\n        return self:{method}()\n    end\nend\n"
    )
}

pub(crate) fn wrappers(native: &Native<'_>) -> Vec<(String, String)> {
    if raw(native) {
        return Vec::new();
    }
    let mut wrappers = vec![(native.place.to_string(), function(native))];
    if let (Some(name), Some((class, method))) = (&native.iterate, native.place.split_once(':')) {
        wrappers.push((format!("{class}:{name}"), iterator(method)));
    }
    wrappers
}
