use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names {
    pub datasets: BTreeSet<String>,
    pub groups: BTreeSet<String>,
}

fn strings_after<'a>(text: &'a str, needle: &str) -> Vec<&'a str> {
    text.match_indices(needle)
        .filter_map(|(i, _)| {
            let rest = text[i + needle.len()..].trim_start();
            let quote = rest.chars().next().filter(|c| *c == '\'' || *c == '"')?;
            let body = &rest[1..];
            let end = body.find(quote)?;
            let value = &body[..end];
            (!value.is_empty() && !value.contains('\n')).then_some(value)
        })
        .collect()
}

pub fn collect(text: &str, names: &mut Names) {
    for d in strings_after(text, "dataset(") {
        names.datasets.insert(d.to_owned());
    }
    for needle in ["->group(", "->groups(", "#[Group("] {
        for g in strings_after(text, needle) {
            names.groups.insert(g.to_owned());
        }
    }
}

pub enum Want {
    Dataset,
    Group,
}

pub fn want_at(text: &str, cursor: usize) -> Option<(usize, usize, Want)> {
    let quote_at = text[..cursor].rfind(['\'', '"'])?;
    if text[quote_at + 1..cursor].contains(['\n', '\'', '"']) {
        return None;
    }
    let before = text[..quote_at].trim_end();
    let want = if before.ends_with("->with(") {
        Want::Dataset
    } else if before.ends_with("->group(")
        || before.ends_with("->groups(")
        || before.ends_with("#[Group(")
    {
        Want::Group
    } else {
        return None;
    };
    let quote = &text[quote_at..quote_at + 1];
    let end = text[cursor..]
        .find(quote)
        .filter(|e| !text[cursor..cursor + e].contains('\n'))
        .map_or(cursor, |e| cursor + e);
    Some((quote_at + 1, end, want))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_datasets_and_groups() {
        let mut names = Names::default();
        collect(
            "<?php\ndataset('emails', ['a@b.c']);\ndataset(\"prices\", fn () => [1]);\n",
            &mut names,
        );
        collect(
            "<?php\nit('works', fn () => 1)->with('emails')->group('slow', 'db');\ntest('x')->groups('api');\n",
            &mut names,
        );
        assert_eq!(
            names
                .datasets
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["emails", "prices"]
        );
        assert_eq!(
            names.groups.iter().map(String::as_str).collect::<Vec<_>>(),
            ["api", "slow"]
        );
    }

    #[test]
    fn knows_what_the_cursor_wants() {
        let text = "it('a')->with('em')->group('sl');";
        assert!(matches!(
            want_at(text, text.find("em").unwrap() + 1),
            Some((_, _, Want::Dataset))
        ));
        assert!(matches!(
            want_at(text, text.find("sl").unwrap() + 1),
            Some((_, _, Want::Group))
        ));
        assert!(want_at(text, text.find("a'").unwrap()).is_none());
    }
}
