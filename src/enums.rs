use crate::casts::{cast_class, file_of, resolve_name};
use crate::modeldoc;
use crate::objects;
use crate::querysql::{Kind, tokenize};
use openidea_plugin::Project;

pub const OPERATORS: [&str; 9] = ["=", "==", "!=", "<>", "<", ">", "<=", ">=", "<=>"];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnumColumn {
    pub column: String,
    pub name: String,
    pub values: Vec<(String, String)>,
}

pub fn of_model(project: &Project, psr4: &[(String, String)], model_text: &str) -> Vec<EnumColumn> {
    modeldoc::casts(model_text)
        .into_iter()
        .filter_map(|(column, expr)| {
            let class = cast_class(&expr)?;
            let fqn = resolve_name(model_text, &class);
            let (_, text) = file_of(project, psr4, &fqn)?;
            let members = objects::read(&text)?;
            (members.kind == "enum" && members.backed.is_some() && !members.values.is_empty())
                .then_some(EnumColumn {
                    column,
                    name: members.name,
                    values: members.values,
                })
        })
        .collect()
}

pub fn call_args(text: &str, open: usize) -> Vec<Option<String>> {
    let Some(rel) = text.get(open..).and_then(|t| t.find('(')) else {
        return Vec::new();
    };
    let from = open + rel;
    let toks = tokenize(&text[from..]);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut current: Vec<usize> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if t.kind == Kind::Sym {
            match t.text.as_str() {
                "(" | "[" | "{" => {
                    depth += 1;
                    if depth == 1 {
                        continue;
                    }
                }
                ")" | "]" | "}" => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                "," if depth == 1 => {
                    out.push(single_string(&toks, &current));
                    current.clear();
                    continue;
                }
                _ => {}
            }
        }
        if depth >= 1 {
            current.push(i);
        }
    }
    out.push(single_string(&toks, &current));
    out
}

fn single_string(toks: &[crate::querysql::Tok], at: &[usize]) -> Option<String> {
    match at {
        [i] if toks[*i].kind == Kind::Str => Some(toks[*i].text.clone()),
        _ => None,
    }
}

pub fn value_column(args: &[Option<String>], index: usize) -> Option<&str> {
    let column = args.first()?.as_deref()?;
    match index {
        1 => Some(column),
        2 => args
            .get(1)?
            .as_deref()
            .filter(|op| OPERATORS.contains(op))
            .map(|_| column),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_literal_arguments_of_a_call() {
        let text = "User::where('status', '!=', 'refu";
        let open = text.find('(').unwrap();
        assert_eq!(
            call_args(text, open),
            [
                Some("status".to_owned()),
                Some("!=".to_owned()),
                Some("refu".to_owned())
            ]
        );
        let args = call_args("x->whereIn('status', ['a', 'b'])", 0);
        assert_eq!(args, [Some("status".to_owned()), None]);
        assert_eq!(
            value_column(&call_args("where('status', '", 0), 1),
            Some("status")
        );
        assert_eq!(
            value_column(&call_args("where('status', '<>', '", 0), 2),
            Some("status")
        );
        assert_eq!(
            value_column(&call_args("where('status', 'x', '", 0), 2),
            None
        );
    }
}
