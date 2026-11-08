use crate::audit::{self, Logged};
use openidea_plugin::{Error, Position, Project, Range, host};

pub fn plain_path(path: &str) -> std::borrow::Cow<'_, str> {
    if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
        return format!("\\\\{rest}").into();
    }
    path.strip_prefix("\\\\?\\").unwrap_or(path).into()
}

fn prefix_len(root: &str, path: &str) -> Option<usize> {
    if path.starts_with(root) {
        return Some(root.len());
    }
    let canon = |s: &str| {
        let s = plain_path(s).replace('\\', "/");
        if crate::runner::is_windows_root(&s) || s.starts_with("//") {
            s.to_ascii_lowercase()
        } else {
            s
        }
    };
    let (root_c, path_c) = (canon(root), canon(path));
    let rest = path_c.strip_prefix(root_c.trim_end_matches('/'))?;
    if !(rest.is_empty() || rest.starts_with('/')) {
        return None;
    }
    let at = path.len().checked_sub(rest.len())?;
    path.is_char_boundary(at).then_some(at)
}

pub fn same_path(a: &str, b: &str) -> bool {
    prefix_len(a, b).is_some_and(|at| at == b.len())
}

pub fn project_of(path: &str) -> Option<Project> {
    host::projects()
        .or_warn("listing the open projects")?
        .into_iter()
        .filter(|p| p.root().is_ok_and(|r| prefix_len(&r, path).is_some()))
        .max_by_key(|p| p.root().map(|r| r.len()).unwrap_or(0))
}

pub fn rel<'a>(root: &str, path: &'a str) -> &'a str {
    prefix_len(root, path).map_or(path, |len| path[len..].trim_start_matches(['/', '\\']))
}

pub fn abs(root: &str, path: &str) -> String {
    if path.starts_with(['/', '\\']) || path.get(1..2) == Some(":") || path.starts_with(root) {
        path.to_owned()
    } else {
        join(root, path)
    }
}

pub fn join(root: &str, relative: &str) -> String {
    let sep = if root.contains('\\') { '\\' } else { '/' };
    let relative = relative.replace(['/', '\\'], &sep.to_string());
    format!("{}{sep}{relative}", root.trim_end_matches(['/', '\\']))
}

pub fn find_some(project: &Project, root: &str, glob: &str, limit: u32) -> Vec<String> {
    project
        .find_files(glob, limit)
        .or_warn(&format!("finding {glob}"))
        .unwrap_or_default()
        .into_iter()
        .map(|p| abs(root, &p))
        .collect()
}

pub fn find(project: &Project, root: &str, glob: &str, limit: u32) -> Vec<String> {
    let found = find_some(project, root, glob, limit);
    if limit > 1 && found.len() >= limit as usize {
        audit::warn(
            "index",
            format!("{glob} matched {limit} files or more, so only the first {limit} were read"),
        );
    }
    found
}

pub fn read_listed(project: &Project, path: &str) -> Result<String, Error> {
    project
        .read_file(path)
        .inspect_err(|e| audit::warn(&format!("reading {path}"), e))
}

pub fn is_laravel(project: &Project) -> bool {
    project.read_file("artisan").is_ok()
        && project
            .read_file("composer.json")
            .is_ok_and(|c| c.contains("laravel/framework") || c.contains("laravel/laravel"))
}

pub fn laravel_version(project: &Project) -> Option<String> {
    let lock = project.read_file("composer.lock").ok()?;
    let at = lock.find("\"name\": \"laravel/framework\"")?;
    let rest = &lock[at..];
    let v = rest.find("\"version\": \"")? + 12;
    let end = rest[v..].find('"')? + v;
    Some(rest[v..end].trim_start_matches('v').to_owned())
}

pub struct Lines {
    starts: Vec<usize>,
}

impl Lines {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { starts }
    }

    pub fn position(&self, text: &str, at: usize) -> Position {
        let at = at.min(text.len());
        let line = self.starts.partition_point(|&s| s <= at) - 1;
        let start = self.starts[line];
        let character = text[start..at].chars().map(|c| c.len_utf16() as u32).sum();
        Position::new(line as u32, character)
    }

    pub fn range(&self, text: &str, start: usize, end: usize) -> Range {
        Range::new(self.position(text, start), self.position(text, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_match_their_root_whatever_the_separators() {
        let root = "C:\\Users\\Pedro\\projects\\app";
        assert_eq!(
            rel(root, "C:/Users/Pedro/projects/app/app/Models/Order.php"),
            "app/Models/Order.php"
        );
        assert_eq!(
            rel(root, "c:\\users\\pedro\\projects\\app\\routes\\web.php"),
            "routes\\web.php"
        );
        assert_eq!(
            rel(root, "C:/Users/Pedro/projects/app2/x.php"),
            "C:/Users/Pedro/projects/app2/x.php"
        );
        assert_eq!(
            rel("/home/p/app", "/home/p/app/routes/web.php"),
            "routes/web.php"
        );
        assert_eq!(
            rel("/home/p/app", "/home/p/App/routes/web.php"),
            "/home/p/App/routes/web.php"
        );
        assert_eq!(
            rel(
                root,
                "\\\\?\\C:\\Users\\Pedro\\projects\\app\\app\\Models\\Order.php"
            ),
            "app\\Models\\Order.php"
        );
        assert_eq!(
            rel(
                "\\\\srv\\share\\app",
                "\\\\?\\UNC\\srv\\share\\app\\routes\\web.php"
            ),
            "routes\\web.php"
        );
        assert!(same_path(
            "C:\\Users\\Pedro\\projects\\app\\a.php",
            "\\\\?\\C:\\Users\\Pedro\\projects\\app\\a.php"
        ));
        assert_eq!(plain_path("\\\\?\\C:\\x\\a.php"), "C:\\x\\a.php");
    }

    #[test]
    fn maps_offsets_to_utf16_positions() {
        let text = "ab\nçé𝄞x\n";
        let lines = Lines::new(text);
        assert_eq!(lines.position(text, 0), Position::new(0, 0));
        assert_eq!(lines.position(text, 3), Position::new(1, 0));
        let x = text.find('x').unwrap();
        assert_eq!(lines.position(text, x), Position::new(1, 4));
        assert_eq!(lines.position(text, text.len()), Position::new(2, 0));
    }

    #[test]
    fn joins_relative_paths() {
        assert_eq!(abs("/p", "config/app.php"), "/p/config/app.php");
        assert_eq!(abs("/p", "/p/config/app.php"), "/p/config/app.php");
        assert_eq!(rel("/p", "/p/routes/web.php"), "routes/web.php");
        let unc = r"\\?\UNC\wsl.localhost\Ubuntu\home\dev\app";
        assert_eq!(join(unc, ".env"), format!(r"{unc}\.env"));
        assert_eq!(abs(unc, "config/app.php"), format!(r"{unc}\config\app.php"));
        assert_eq!(
            rel(unc, &format!(r"{unc}\routes\web.php")),
            r"routes\web.php"
        );
    }
}
