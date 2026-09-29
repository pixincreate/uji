use syn::ext::IdentExt;
use syn::{BareFnArg, Ident, ReturnType, Type, TypeBareFn, TypePtr};

pub(crate) fn key(ident: &Ident) -> String {
    ident.unraw().to_string()
}

pub(crate) fn c_type(ty: &Type) -> syn::Result<String> {
    match ty {
        Type::Ptr(pointer) => pointer_type(pointer),
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok("void".to_string()),
        Type::Path(path) => {
            let Some(segment) = path.path.segments.last() else {
                return Err(unsupported(ty));
            };
            let name = segment.ident.to_string();
            let known = match name.as_str() {
                "bool" => "bool",
                "i8" => "int8_t",
                "i16" => "int16_t",
                "i32" => "int32_t",
                "i64" => "int64_t",
                "u8" => "uint8_t",
                "u16" => "uint16_t",
                "u32" => "uint32_t",
                "u64" => "uint64_t",
                "usize" => "size_t",
                "isize" => "ptrdiff_t",
                "f32" => "float",
                "f64" => "double",
                _ => return Err(unsupported(ty)),
            };
            Ok(known.to_string())
        }
        _ => Err(unsupported(ty)),
    }
}

pub(crate) fn c_field(name: &str, ty: &Type) -> syn::Result<String> {
    if let Type::BareFn(function) = ty {
        return function_pointer(name, function);
    }
    Ok(format!("{} {name}", c_type(ty)?))
}

pub(crate) fn c_return(output: &ReturnType) -> syn::Result<String> {
    match output {
        ReturnType::Default => Ok("void".to_string()),
        ReturnType::Type(_, ty) => c_type(ty),
    }
}

pub(crate) fn snake(ident: &Ident) -> String {
    let mut name = String::new();
    for char in key(ident).chars() {
        if char.is_uppercase() {
            if !name.is_empty() {
                name.push('_');
            }
            name.extend(char.to_lowercase());
        } else {
            name.push(char);
        }
    }
    name
}

const CORE: [&str; 2] = ["uji_kernel", "uji_native"];

pub(crate) fn struct_name(ident: &Ident) -> String {
    let scope = std::env::var("CARGO_CRATE_NAME")
        .ok()
        .filter(|name| !CORE.contains(&name.as_str()))
        .unwrap_or_else(|| "uji".to_string());
    format!("{scope}_{}", snake(ident))
}

fn pointer_type(pointer: &TypePtr) -> syn::Result<String> {
    let constness = if pointer.mutability.is_some() {
        ""
    } else {
        "const "
    };
    let Type::Path(path) = &*pointer.elem else {
        return Err(unsupported(&pointer.elem));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(unsupported(&pointer.elem));
    };
    let target = match segment.ident.to_string().as_str() {
        "u8" | "c_char" | "i8" => "char".to_string(),
        "c_void" => return Ok("void *".to_string()),
        _ => struct_name(&segment.ident),
    };
    Ok(format!("{constness}{target} *"))
}

fn function_pointer(name: &str, function: &TypeBareFn) -> syn::Result<String> {
    let arguments = function
        .inputs
        .iter()
        .map(|argument: &BareFnArg| c_type(&argument.ty))
        .collect::<syn::Result<Vec<_>>>()?;
    let arguments = if arguments.is_empty() {
        "void".to_string()
    } else {
        arguments.join(", ")
    };
    Ok(format!(
        "{} (*{name})({arguments})",
        c_return(&function.output)?
    ))
}

fn unsupported(ty: &Type) -> syn::Error {
    syn::Error::new_spanned(ty, "this type has no C form for a native function")
}
