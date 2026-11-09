use crate::hierarchy::{self, ProjectSymbol};
use crate::project::{find, read_listed, rel};
use crate::resources::{self, Scan};
use crate::usages::{self, GLOBS};
use openidea_plugin::Project;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

const LIMIT: u32 = 20_000;

pub fn scanned(rel_path: &str) -> bool {
    rel_path.ends_with(".php")
        && !rel_path.ends_with(".blade.php")
        && ([
            "app/",
            "database/factories/",
            "database/seeders/",
            "routes/",
        ]
        .iter()
        .any(|dir| rel_path.starts_with(dir))
            || matches!(rel_path, "bootstrap/app.php" | "bootstrap/providers.php"))
}

#[derive(Default)]
pub struct Store {
    listed: bool,
    pending: Vec<String>,
    usages: usages::Index,
    scans: BTreeMap<String, Scan>,
    file_symbols: BTreeMap<String, Vec<ProjectSymbol>>,
    merged: Option<Rc<Scan>>,
    symbols: Option<Rc<Vec<ProjectSymbol>>>,
}

impl Store {
    pub fn ready(&self) -> bool {
        self.listed && self.pending.is_empty()
    }

    fn list(&mut self, project: &Project, root: &str) {
        let mut paths: Vec<String> = GLOBS
            .iter()
            .flat_map(|glob| find(project, root, glob, LIMIT))
            .map(|p| rel(root, &p).replace('\\', "/"))
            .filter(|r| !r.starts_with("bootstrap/cache/"))
            .collect();
        paths.sort();
        paths.dedup();
        paths.reverse();
        self.pending = paths;
        self.listed = true;
    }

    fn put(&mut self, rel_path: &str, text: Option<&str>) {
        self.merged = None;
        self.symbols = None;
        match text {
            Some(text) => {
                self.usages.set(rel_path, text);
                if scanned(rel_path) {
                    let scan = resources::scan_text(rel_path, text);
                    self.file_symbols
                        .insert(rel_path.to_owned(), hierarchy::project_symbols(&scan));
                    self.scans.insert(rel_path.to_owned(), scan);
                }
            }
            None => {
                self.usages.remove(rel_path);
                self.scans.remove(rel_path);
                self.file_symbols.remove(rel_path);
            }
        }
    }

    pub fn advance(&mut self, project: &Project, root: &str, budget: Duration) -> bool {
        let started = Instant::now();
        if !self.listed {
            self.list(project, root);
        }
        while let Some(rel_path) = self.pending.pop() {
            let text = read_listed(project, &crate::project::join(root, &rel_path)).ok();
            self.put(&rel_path, text.as_deref());
            if started.elapsed() >= budget {
                break;
            }
        }
        self.ready()
    }

    pub fn update(&mut self, project: &Project, root: &str, path: &str) {
        let rel_path = rel(root, path).replace('\\', "/");
        if !usages::indexed(&rel_path) {
            return;
        }
        self.pending.retain(|p| *p != rel_path);
        let text = project.read_file(path).ok();
        self.put(&rel_path, text.as_deref());
    }

    pub fn files_using(&self, word: &str, except: &str) -> Vec<(String, u32)> {
        self.usages.files_using(word, except)
    }

    pub fn scan(&mut self) -> Rc<Scan> {
        if let Some(merged) = &self.merged {
            return merged.clone();
        }
        let mut out = Scan::default();
        for part in self.scans.values() {
            out.symbols.extend(part.symbols.iter().cloned());
            out.calls.extend(part.calls.iter().cloned());
        }
        let merged = Rc::new(out);
        self.merged = Some(merged.clone());
        merged
    }

    pub fn symbols(&mut self) -> Rc<Vec<ProjectSymbol>> {
        if let Some(symbols) = &self.symbols {
            return symbols.clone();
        }
        let symbols: Rc<Vec<ProjectSymbol>> =
            Rc::new(self.file_symbols.values().flatten().cloned().collect());
        self.symbols = Some(symbols.clone());
        symbols
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    #[test]
    fn reads_the_project_in_slices_and_updates_one_file() {
        let host = MockHost::install();
        host.add_file("artisan", "");
        host.add_file(
            "app/Models/User.php",
            "<?php\nnamespace App\\Models;\nclass User extends Model {}\n",
        );
        host.add_file(
            "app/Http/Controllers/A.php",
            "<?php\nnamespace App\\Http\\Controllers;\nclass A { public function b() { return User::all(); } }\n",
        );
        host.add_file("tests/Feature/UserTest.php", "<?php\nUser::factory();\n");
        let project = crate::project::project_of("/project/artisan").unwrap();
        let mut store = Store::default();
        assert!(!store.advance(&project, "/project", Duration::ZERO));
        while !store.advance(&project, "/project", Duration::ZERO) {}
        assert_eq!(store.files_using("User", "app/Models/User.php").len(), 2);
        assert_eq!(store.scan().symbols.len(), 2);
        assert_eq!(store.symbols().len(), 2);
        host.add_file(
            "app/Http/Controllers/A.php",
            "<?php\nnamespace App\\Http\\Controllers;\nclass A {}\n",
        );
        store.update(&project, "/project", "/project/app/Http/Controllers/A.php");
        assert_eq!(store.files_using("User", "app/Models/User.php").len(), 1);
        assert_eq!(store.scan().symbols.len(), 2);
    }
}
