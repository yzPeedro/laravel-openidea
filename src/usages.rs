use crate::project::Lines;
use std::collections::BTreeMap;

pub const GLOBS: [&str; 8] = [
    "app/**/*.php",
    "database/**/*.php",
    "routes/**/*.php",
    "tests/**/*.php",
    "config/**/*.php",
    "resources/views/**/*.php",
    "bootstrap/app.php",
    "bootstrap/providers.php",
];

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

fn blank(out: &mut [u8], from: usize, to: usize) {
    for b in &mut out[from..to] {
        if *b != b'\n' {
            *b = b' ';
        }
    }
}

fn starts_statement(code: &[u8], at: usize) -> bool {
    let mut end = at;
    while end > 0 && code[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let before = &code[..end];
    before.is_empty()
        || matches!(before.last(), Some(b';' | b'{' | b'}'))
        || before.ends_with(b"<?php")
}

fn clean_blade(text: &str) -> String {
    let mut out = text.as_bytes().to_vec();
    let mut from = 0;
    while let Some(open) = text[from..].find("{{--") {
        let start = from + open;
        let end = text[start + 4..]
            .find("--}}")
            .map_or(text.len(), |i| start + 4 + i + 4);
        blank(&mut out, start, end);
        from = end;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

pub fn clean(text: &str, blade: bool) -> String {
    if blade {
        return clean_blade(text);
    }
    let bytes = text.as_bytes();
    let mut out = bytes.to_vec();
    let mut braces = 0usize;
    let mut parens = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'\'' | b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let end = text[i..].find('\n').map_or(text.len(), |n| i + n);
                blank(&mut out, i, end);
                i = end;
            }
            b'#' if bytes.get(i + 1) != Some(&b'[') => {
                let end = text[i..].find('\n').map_or(text.len(), |n| i + n);
                blank(&mut out, i, end);
                i = end;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let end = text[i + 2..]
                    .find("*/")
                    .map_or(text.len(), |n| i + 2 + n + 2);
                blank(&mut out, i, end);
                i = end;
            }
            b'{' => {
                braces += 1;
                i += 1;
            }
            b'}' => {
                braces = braces.saturating_sub(1);
                i += 1;
            }
            b'(' => {
                parens += 1;
                i += 1;
            }
            b')' => {
                parens = parens.saturating_sub(1);
                i += 1;
            }
            b'u' if braces == 0
                && parens == 0
                && bytes[i..].starts_with(b"use")
                && bytes.get(i + 3).is_some_and(|c| c.is_ascii_whitespace())
                && (i == 0 || !is_word(bytes[i - 1]))
                && starts_statement(&out, i) =>
            {
                let end = text[i..].find(';').map_or(text.len(), |n| i + n + 1);
                blank(&mut out, i, end);
                i = end;
            }
            _ => i += 1,
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

pub fn occurrences(cleaned: &str, word: &str) -> Vec<usize> {
    let bytes = cleaned.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = cleaned[from..].find(word) {
        let at = from + found;
        let end = at + word.len();
        from = end;
        let before = at.checked_sub(1).map(|i| bytes[i]);
        if before.is_some_and(|b| is_word(b) || b == b'$')
            || bytes.get(end).is_some_and(|b| is_word(*b))
        {
            continue;
        }
        out.push(at);
    }
    out
}

fn words(cleaned: &str) -> BTreeMap<String, u32> {
    let bytes = cleaned.as_bytes();
    let mut out = BTreeMap::new();
    let mut line = 1u32;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if !(b.is_ascii_alphabetic() || b == b'_')
            || (i > 0 && (is_word(bytes[i - 1]) || bytes[i - 1] == b'$'))
        {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && is_word(bytes[i]) {
            i += 1;
        }
        let word = &cleaned[start..i];
        let called = cleaned[..start].ends_with("->") || cleaned[..start].ends_with("::");
        if (b.is_ascii_uppercase() || called) && !out.contains_key(word) {
            out.insert(word.to_owned(), line);
        }
    }
    out
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    files: BTreeMap<String, BTreeMap<String, u32>>,
}

pub fn indexed(rel_path: &str) -> bool {
    let rel_path = rel_path.replace('\\', "/");
    rel_path.ends_with(".php")
        && [
            "app/",
            "database/",
            "routes/",
            "tests/",
            "config/",
            "resources/views/",
        ]
        .iter()
        .any(|dir| rel_path.starts_with(dir))
        || matches!(
            rel_path.as_str(),
            "bootstrap/app.php" | "bootstrap/providers.php"
        )
}

impl Index {
    pub fn set(&mut self, rel_path: &str, text: &str) {
        let rel_path = rel_path.replace('\\', "/");
        let cleaned = clean(text, rel_path.ends_with(".blade.php"));
        self.files.insert(rel_path, words(&cleaned));
    }

    pub fn remove(&mut self, rel_path: &str) {
        self.files.remove(&rel_path.replace('\\', "/"));
    }

    pub fn files_using(&self, word: &str, except: &str) -> Vec<(String, u32)> {
        let except = except.replace('\\', "/");
        self.files
            .iter()
            .filter(|(path, _)| **path != except)
            .filter_map(|(path, words)| words.get(word).map(|line| (path.clone(), *line)))
            .collect()
    }
}

pub fn line_at(text: &str, at: usize) -> u32 {
    Lines::new(text).position(text, at).line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_imports_are_blanked_with_offsets_kept() {
        let text = "<?php\nnamespace App;\n\nuse App\\Models\\User;\nuse App\\Models\\{\n    Order,\n    Post\n};\n\n// User here\n# User there\n/** @param User $u */\nclass A\n{\n    use HasFactory;\n    #[Scope]\n    public function f() { return fn () => User::all(); }\n    public function g() { $x = function () use ($y) { return '// User'; }; }\n}\n";
        let cleaned = clean(text, false);
        assert_eq!(cleaned.len(), text.len());
        assert_eq!(cleaned.matches('\n').count(), text.matches('\n').count());
        assert!(!cleaned.contains("App\\Models\\User;"));
        assert!(!cleaned.contains("Order"));
        assert!(!cleaned.contains("User here") && !cleaned.contains("User there"));
        assert!(!cleaned.contains("@param"));
        assert!(cleaned.contains("use HasFactory;"));
        assert!(cleaned.contains("#[Scope]"));
        assert!(cleaned.contains("use ($y)"));
        assert!(cleaned.contains("'// User'"));
        assert_eq!(occurrences(&cleaned, "User").len(), 2);
    }

    #[test]
    fn routes_keep_closure_imports_and_drop_class_imports() {
        let text = "<?php\n\nuse App\\Models\\User;\nuse Illuminate\\Support\\Facades\\Route;\n\nRoute::get('/', function () use ($x) { return User::all(); });\n";
        let cleaned = clean(text, false);
        assert!(cleaned.contains("use ($x)"));
        let mut index = Index::default();
        index.set("routes/web.php", text);
        assert_eq!(
            index.files_using("User", ""),
            [("routes/web.php".to_owned(), 6)]
        );
        assert_eq!(
            index.files_using("get", ""),
            [("routes/web.php".to_owned(), 6)]
        );
    }

    #[test]
    fn blade_comments_are_blanked() {
        let text = "{{-- User::all() --}}\n<a href=\"#\">{{ App\\Models\\Post::count() }}</a>\n";
        let mut index = Index::default();
        index.set("resources/views/a.blade.php", text);
        assert!(index.files_using("User", "").is_empty());
        assert_eq!(index.files_using("Post", "").len(), 1);
    }

    #[test]
    fn only_project_sources_are_indexed() {
        assert!(indexed("app/Models/User.php"));
        assert!(indexed("routes\\web.php"));
        assert!(!indexed("vendor/laravel/framework/src/A.php"));
        assert!(!indexed("storage/openidea/ide-helpers.php"));
    }
}
