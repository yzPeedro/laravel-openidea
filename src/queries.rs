use crate::Laravel;
use crate::devtools;
use crate::index::RefKind;
use crate::querysql::{self, Model};
use crate::runner;
use openidea_plugin::text::offset;
use openidea_plugin::{Error, Invocation, Project, host};

fn short(class: &str) -> &str {
    class.rsplit('\\').next().unwrap_or(class)
}

fn editor_code(ctx: &Invocation, whole_statement: bool) -> Result<(String, String), Error> {
    let editor = ctx
        .editor()
        .ok_or_else(|| Error::Invalid("Open a PHP file in the editor first.".into()))?;
    let doc = editor.document()?;
    let sel = editor
        .selections()?
        .into_iter()
        .next()
        .ok_or_else(|| Error::Invalid("Place the cursor in the editor first.".into()))?;
    let (a, b) = (
        offset(&doc.text, sel.start).unwrap_or(0),
        offset(&doc.text, sel.end).unwrap_or(0),
    );
    let (a, b) = (a.min(b), a.max(b));
    let code = if a < b {
        doc.text[a..b].to_owned()
    } else if whole_statement {
        querysql::statement_at(&doc.text, a).to_owned()
    } else {
        String::new()
    };
    if code.trim().is_empty() {
        return Err(Error::Invalid(
            "Select the code first, or place the cursor in a query.".into(),
        ));
    }
    Ok((code, editor.path()))
}

impl Laravel {
    pub(crate) fn query_models(&mut self, project: &Project, root: &str) -> Vec<(String, Model)> {
        self.cache
            .get(RefKind::Model, project, root)
            .iter()
            .filter_map(|e| {
                let table = e.detail.as_deref()?.strip_prefix("table ")?.to_owned();
                let source =
                    crate::project::read_listed(project, &e.location.path).unwrap_or_default();
                Some((
                    e.key.clone(),
                    Model {
                        table,
                        soft_deletes: source.contains("SoftDeletes"),
                        scopes: querysql::scopes_of(&source),
                    },
                ))
            })
            .collect()
    }

    pub(crate) fn model_classes(&mut self, project: &Project, root: &str) -> Vec<String> {
        self.cache
            .get(RefKind::Model, project, root)
            .iter()
            .filter_map(|e| {
                let source = crate::project::read_listed(project, &e.location.path).ok()?;
                let namespace = source
                    .lines()
                    .find_map(|l| l.trim().strip_prefix("namespace "))
                    .map(|n| n.trim_end_matches(';').trim().to_owned())?;
                Some(format!("{namespace}\\{}", short(&e.key)))
            })
            .collect()
    }

    pub(crate) fn php_to_sql(
        &mut self,
        project: Option<(&Project, &str)>,
        code: &str,
    ) -> Result<querysql::Output, String> {
        let models = match project {
            Some((p, root)) => self.query_models(p, root),
            None => Vec::new(),
        };
        let lookup = |name: &str| {
            models
                .iter()
                .find(|(k, _)| short(k) == short(name))
                .map(|(_, m)| m.clone())
        };
        querysql::to_sql(code, &lookup)
    }

    pub(crate) fn copy_query_sql(&mut self, ctx: &Invocation) -> Result<Option<String>, Error> {
        let (code, path) = editor_code(ctx, true)?;
        let project = self.laravel_project(&path);
        let out = self
            .php_to_sql(project.as_ref().map(|(p, r)| (p, r.as_str())), &code)
            .map_err(Error::Invalid)?;
        match host::copy_to_clipboard(&out.sql) {
            Ok(()) => {}
            Err(Error::Denied(cap)) => {
                return Err(Error::Failed(format!(
                    "Allow \"{cap}\" for Laravel OpenIDEA in Settings › Plugins to copy queries."
                )));
            }
            Err(e) => return Err(e),
        }
        let mut message = "Copied the query as MySQL.".to_owned();
        for note in out.notes {
            message.push(' ');
            message.push_str(&note);
            message.push('.');
        }
        Ok(Some(message))
    }

    pub(crate) fn run_selection_in_tinker(
        &mut self,
        ctx: &Invocation,
    ) -> Result<Option<String>, Error> {
        let (code, path) = editor_code(ctx, false)?;
        let (project, root) = self
            .laravel_project(&path)
            .ok_or("this file is not in a Laravel project")?;
        let source = ctx
            .editor()
            .and_then(|e| e.document().ok())
            .map(|d| d.text)
            .unwrap_or_default();
        let classes = self.model_classes(&project, &root);
        let file = querysql::tinker_file(&code, &source, &classes);
        let mode = runner::detect(&project);
        let write = mode.php_args(&["-r".into(), devtools::script(devtools::SELECTION_PHP)]);
        let written = match crate::runner::run(&project, &mode, &write, Some(&file)) {
            Ok(out) => out,
            Err(Error::Denied(cap)) => {
                return Err(Error::Failed(format!(
                    "Allow \"{cap}\" for Laravel OpenIDEA in Settings › Plugins to run Tinker."
                )));
            }
            Err(e) => return Err(e),
        };
        if written.status != Some(0) {
            let why = written.stderr.trim();
            return Err(Error::Failed(format!(
                "Couldn't prepare the selection for Tinker: {}",
                if why.is_empty() {
                    written.stdout.trim()
                } else {
                    why
                }
            )));
        }
        let args = mode.args(
            &["tinker".into(), "storage/openidea/selection.php".into()],
            true,
        );
        match host::open_terminal("artisan tinker (selection)", mode.program(), &args) {
            Ok(()) => Ok(None),
            Err(Error::Denied(cap)) => Err(Error::Failed(format!(
                "Allow \"{cap}\" for Laravel OpenIDEA in Settings › Plugins to run Tinker."
            ))),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Laravel;
    use openidea_plugin::Plugin as _;
    use openidea_plugin::mock::MockHost;

    fn laravel_app() -> MockHost {
        let host = MockHost::install();
        host.add_file("artisan", "#!/usr/bin/env php\n");
        host.add_file(
            "composer.json",
            "{\"require\": {\"laravel/framework\": \"^12.0\"}}",
        );
        host.add_file(
            "app/Models/Post.php",
            "<?php\nnamespace App\\Models;\nuse Illuminate\\Database\\Eloquent\\Model;\nuse Illuminate\\Database\\Eloquent\\SoftDeletes;\nclass Post extends Model\n{\n    use SoftDeletes;\n}\n",
        );
        host
    }

    #[test]
    fn copies_the_query_under_the_cursor_as_mysql() {
        let host = laravel_app();
        host.open_editor(
            "/project/app/Http/Controllers/A.php",
            "php",
            "<?php\nclass A {\n    public function index()\n    {\n        return Post::where('id', $id)\n            ->first();\n    }\n}\n",
            &[(4, 22, 4, 22)],
        );
        let mut plugin = Laravel::default();
        let message = plugin
            .run_command("copy-query-sql", &host.invocation(), None)
            .unwrap()
            .unwrap();
        assert_eq!(
            host.clipboard().as_deref(),
            Some(
                "SELECT *\nFROM `posts`\nWHERE `id` = ?\n  AND `posts`.`deleted_at` IS NULL\nLIMIT 1"
            )
        );
        assert!(message.contains("? stands for $id"), "{message}");
    }

    #[test]
    fn copying_without_the_clipboard_grant_says_what_to_allow() {
        let host = laravel_app().with_capabilities(&["project.read", "editor.read"]);
        host.open_editor(
            "/project/app/A.php",
            "php",
            "<?php\nPost::count();\n",
            &[(1, 3, 1, 3)],
        );
        let err = Laravel::default()
            .run_command("copy-query-sql", &host.invocation(), None)
            .unwrap_err();
        assert!(err.to_string().contains("clipboard"), "{err}");
    }

    #[test]
    fn runs_the_selection_in_tinker_with_each_value_dumped() {
        let host = laravel_app().with_capabilities(&["project.read", "editor.read", "process:php"]);
        let written = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let sink = written.clone();
        host.on_process("php", move |_, stdin| {
            *sink.borrow_mut() = stdin.unwrap_or_default().to_owned();
            Ok(openidea_plugin::ProcessOutput {
                status: Some(0),
                stdout: "ok\n".into(),
                stderr: String::new(),
            })
        });
        let text = "<?php\n$count = Post::count();\nPost::first();\n";
        host.open_editor("/project/routes/web.php", "php", text, &[(1, 0, 2, 14)]);
        Laravel::default()
            .run_command("run-in-tinker", &host.invocation(), None)
            .unwrap();
        assert_eq!(
            written.borrow().as_str(),
            "<?php\n\nuse App\\Models\\Post;\n\ndump($count = Post::count());\ndump(Post::first());\n"
        );
        let terminals = host.terminals();
        let t = terminals.last().unwrap();
        assert_eq!(t.program, "php");
        assert_eq!(
            t.args,
            ["artisan", "tinker", "storage/openidea/selection.php"]
        );
    }

    #[test]
    fn running_in_tinker_needs_a_selection() {
        let host = laravel_app();
        host.open_editor(
            "/project/routes/web.php",
            "php",
            "<?php\nPost::count();\n",
            &[(1, 2, 1, 2)],
        );
        let err = Laravel::default()
            .run_command("run-in-tinker", &host.invocation(), None)
            .unwrap_err();
        assert!(err.to_string().contains("Select the code"), "{err}");
    }
}
