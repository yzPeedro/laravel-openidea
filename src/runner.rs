use openidea_plugin::{Error, ProcessOutput, Project, host};

const COMPOSE_FILES: [&str; 4] = [
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];
const PREFERRED_SERVICES: [&str; 6] = ["app", "laravel.test", "laravel", "php", "api", "web"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Php,
    DockerRun(String),
    DockerExec(String),
}

impl Mode {
    pub fn label(&self) -> String {
        match self {
            Mode::Php => "php".into(),
            Mode::DockerRun(s) => format!("docker compose run --rm {s}"),
            Mode::DockerExec(s) => format!("docker compose exec {s}"),
        }
    }

    pub fn program(&self) -> &'static str {
        match self {
            Mode::Php => "php",
            _ => "docker",
        }
    }

    fn prefix(&self, interactive: bool) -> Vec<String> {
        match self {
            Mode::Php => Vec::new(),
            Mode::DockerRun(service) => {
                let mut a = vec![
                    "compose".into(),
                    "run".into(),
                    "--rm".into(),
                    "--no-deps".into(),
                ];
                if !interactive {
                    a.push("-T".into());
                }
                a.extend(["--entrypoint".into(), "php".into(), service.clone()]);
                a
            }
            Mode::DockerExec(service) => {
                let mut a = vec!["compose".into(), "exec".into()];
                if !interactive {
                    a.push("-T".into());
                }
                a.extend([service.clone(), "php".into()]);
                a
            }
        }
    }

    pub fn args(&self, argv: &[String], interactive: bool) -> Vec<String> {
        let mut args = self.prefix(interactive);
        args.push("artisan".into());
        args.extend(argv.iter().cloned());
        args
    }

    pub fn php_args(&self, argv: &[String]) -> Vec<String> {
        let mut args = self.prefix(false);
        args.extend(argv.iter().cloned());
        args
    }
}

pub const WINDOWLESS_PHP: &str = "php-win";

pub fn is_windows_root(root: &str) -> bool {
    root.as_bytes().get(1) == Some(&b':') || root.starts_with("\\\\")
}

fn unavailable(error: &Error) -> bool {
    match error {
        Error::Denied(_) | Error::NotFound(_) => true,
        Error::Failed(message) => message.starts_with(&format!("process:{WINDOWLESS_PHP}")),
        _ => false,
    }
}

const INLINE_CODE_BYTES: usize = 1024;

pub const BOOT: &str = "$__oiN=(int)fgets(STDIN);$__oiC='';while(strlen($__oiC)<$__oiN&&!feof(STDIN)){$__oiC.=fread(STDIN,$__oiN-strlen($__oiC));}unset($__oiN);eval($__oiC);";

pub fn pack(args: &[String], stdin: Option<&str>) -> (Vec<String>, Option<String>) {
    let long = args
        .iter()
        .position(|a| a == "-r")
        .filter(|&i| args.get(i + 1).is_some_and(|c| c.len() > INLINE_CODE_BYTES));
    let Some(at) = long else {
        return (args.to_vec(), stdin.map(str::to_owned));
    };
    let code = &args[at + 1];
    let mut packed = args.to_vec();
    packed[at + 1] = BOOT.to_owned();
    let input = format!("{}\n{code}{}", code.len(), stdin.unwrap_or(""));
    (packed, Some(input))
}

pub fn run(
    project: &Project,
    mode: &Mode,
    args: &[String],
    stdin: Option<&str>,
) -> Result<ProcessOutput, Error> {
    let (args, stdin) = pack(args, stdin);
    let stdin = stdin.as_deref();
    if *mode == Mode::Php && project.root().is_ok_and(|r| is_windows_root(&r)) {
        match project.run_process(WINDOWLESS_PHP, &args, stdin) {
            Err(e) if unavailable(&e) => {}
            other => return other,
        }
    }
    project.run_process(mode.program(), &args, stdin)
}

impl Mode {
    pub fn mcp_server(&self, root: &str, compose: Option<&str>) -> (String, Vec<String>) {
        let script = "storage/openidea/mcp.php".to_owned();
        let file = |c: Option<&str>| {
            c.map(|f| {
                vec![
                    "-f".to_owned(),
                    format!("{}/{f}", root.trim_end_matches('/')),
                ]
            })
            .unwrap_or_default()
        };
        match self {
            Mode::Php => (
                "php".into(),
                vec![format!("{}/{script}", root.trim_end_matches('/'))],
            ),
            Mode::DockerRun(service) => {
                let mut a = vec!["compose".to_owned()];
                a.extend(file(compose));
                a.extend(
                    ["run", "--rm", "--no-deps", "-T", "--entrypoint", "php"].map(str::to_owned),
                );
                a.extend([service.clone(), script]);
                ("docker".into(), a)
            }
            Mode::DockerExec(service) => {
                let mut a = vec!["compose".to_owned()];
                a.extend(file(compose));
                a.extend([
                    "exec".to_owned(),
                    "-T".to_owned(),
                    service.clone(),
                    "php".to_owned(),
                    script,
                ]);
                ("docker".into(), a)
            }
        }
    }

    pub fn composer(&self, argv: &[String]) -> (&'static str, Vec<String>) {
        match self {
            Mode::Php => {
                let quoted: Vec<String> = argv
                    .iter()
                    .map(|a| format!("'{}'", a.replace('\'', "")))
                    .collect();
                (
                    "php",
                    vec![
                        "-r".into(),
                        format!(
                            "passthru('composer ' . \"{}\", $code); exit($code);",
                            quoted.join(" ").replace('"', "")
                        ),
                    ],
                )
            }
            Mode::DockerRun(service) => {
                let mut a: Vec<String> = [
                    "compose",
                    "run",
                    "--rm",
                    "--no-deps",
                    "--entrypoint",
                    "composer",
                ]
                .map(str::to_owned)
                .to_vec();
                a.push(service.clone());
                a.extend(argv.iter().cloned());
                ("docker", a)
            }
            Mode::DockerExec(service) => {
                let mut a = vec![
                    "compose".to_owned(),
                    "exec".to_owned(),
                    service.clone(),
                    "composer".to_owned(),
                ];
                a.extend(argv.iter().cloned());
                ("docker", a)
            }
        }
    }
}

pub fn compose_file(project: &Project) -> Option<&'static str> {
    COMPOSE_FILES
        .iter()
        .copied()
        .find(|f| project.read_file(f).is_ok())
}

pub fn services(compose: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in compose.lines() {
        if line.trim_end() == "services:" {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if !line.starts_with(' ') && !line.trim().is_empty() && !line.starts_with('#') {
            break;
        }
        let indent = line.len() - line.trim_start().len();
        let name = line.trim();
        if indent == 2 && name.ends_with(':') && !name.starts_with('#') {
            out.push(
                name.trim_end_matches(':')
                    .trim_matches(['"', '\''])
                    .to_owned(),
            );
        }
    }
    out
}

pub fn detect(project: &Project) -> Mode {
    let configured = host::setting_string("artisanRunner").unwrap_or_default();
    let service_setting = host::setting_string("dockerService").filter(|s| !s.is_empty());
    let compose = COMPOSE_FILES.iter().find_map(|f| project.read_file(f).ok());
    let service = || {
        service_setting.clone().unwrap_or_else(|| {
            let found = compose.as_deref().map(services).unwrap_or_default();
            PREFERRED_SERVICES
                .iter()
                .find(|p| found.iter().any(|s| s == *p))
                .map(|s| (*s).to_owned())
                .or_else(|| found.first().cloned())
                .unwrap_or_else(|| "app".into())
        })
    };
    match configured.as_str() {
        "php" => Mode::Php,
        "docker" | "docker-exec" => Mode::DockerExec(service()),
        "docker-run" => Mode::DockerRun(service()),
        _ if compose.is_some() => Mode::DockerRun(service()),
        _ => Mode::Php,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    #[test]
    fn long_scripts_travel_on_stdin_and_still_read_their_input() {
        let script = format!(
            "$pad = '{}'; echo $argv[1], ':', strlen($pad), ':', stream_get_contents(STDIN);",
            "x".repeat(5000)
        );
        let args = vec!["-r".to_owned(), script.clone(), "hub".to_owned()];
        let (packed, stdin) = pack(&args, Some("payload ç"));
        assert_eq!(packed, ["-r", BOOT, "hub"]);
        assert!(packed.iter().all(|a| a.len() <= 4096));
        assert!(
            stdin
                .as_deref()
                .unwrap()
                .starts_with(&format!("{}\n", script.len()))
        );
        let short = vec!["-r".to_owned(), "echo 1;".to_owned()];
        assert_eq!(pack(&short, None), (short.clone(), None));
        let Ok(mut child) = std::process::Command::new("php")
            .args(&packed)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
        else {
            return;
        };
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.unwrap().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "hub:5000:payload ç");
    }

    fn ran(host: &MockHost) -> Vec<String> {
        host.process_runs().into_iter().map(|r| r.program).collect()
    }

    #[test]
    fn windows_projects_run_php_without_a_console_window_when_allowed() {
        let ok = |_: &[String], _: Option<&str>| {
            Ok(ProcessOutput {
                status: Some(0),
                stdout: "ok".into(),
                stderr: String::new(),
            })
        };
        let host = MockHost::install().with_capabilities(&[
            "project.read",
            "process:php",
            "process:php-win",
        ]);
        host.set_project_root(Some("C:\\Users\\dev\\shop"));
        host.on_process("php", ok);
        host.on_process("php-win", ok);
        let project = host::projects().unwrap().remove(0);
        let args = ["-v".to_owned()];
        assert_eq!(run(&project, &Mode::Php, &args, None).unwrap().stdout, "ok");
        assert_eq!(ran(&host), ["php-win"]);

        let host = MockHost::install().with_capabilities(&["project.read", "process:php"]);
        host.set_project_root(Some("C:\\Users\\dev\\shop"));
        host.on_process("php", ok);
        let project = host::projects().unwrap().remove(0);
        run(&project, &Mode::Php, &args, None).unwrap();
        assert_eq!(ran(&host), ["php"]);

        let host = MockHost::install().with_capabilities(&[
            "project.read",
            "process:php",
            "process:php-win",
        ]);
        host.on_process("php", ok);
        host.on_process("php-win", ok);
        let project = host::projects().unwrap().remove(0);
        run(&project, &Mode::Php, &args, None).unwrap();
        assert_eq!(ran(&host), ["php"]);
        assert!(is_windows_root("\\\\?\\UNC\\wsl.localhost\\Ubuntu\\app"));
        assert!(!is_windows_root("/home/dev/app"));
    }

    #[test]
    fn reads_compose_services_and_builds_commands() {
        let compose = "name: shop\nservices:\n  mysql:\n    image: mysql\n  app:\n    build: .\n    environment:\n      A: b\nvolumes:\n  data:\n";
        assert_eq!(services(compose), ["mysql", "app"]);
        let run = Mode::DockerRun("app".into());
        assert_eq!(
            run.args(&["migrate:status".into()], false),
            [
                "compose",
                "run",
                "--rm",
                "--no-deps",
                "-T",
                "--entrypoint",
                "php",
                "app",
                "artisan",
                "migrate:status"
            ]
        );
        assert_eq!(
            Mode::Php.args(&["about".into()], true),
            ["artisan", "about"]
        );
    }

    #[test]
    fn builds_mcp_and_composer_commands() {
        let (cmd, args) = Mode::Php.mcp_server("/p", None);
        assert_eq!(
            (cmd.as_str(), args),
            ("php", vec!["/p/storage/openidea/mcp.php".to_owned()])
        );
        let (cmd, args) = Mode::DockerRun("app".into()).mcp_server("/p", Some("compose.yaml"));
        assert_eq!(cmd, "docker");
        assert_eq!(
            args.join(" "),
            "compose -f /p/compose.yaml run --rm --no-deps -T --entrypoint php app storage/openidea/mcp.php"
        );
        let argv = [
            "require".to_owned(),
            "laravel/boost".to_owned(),
            "--dev".to_owned(),
        ];
        let (program, args) = Mode::DockerRun("app".into()).composer(&argv);
        assert_eq!(program, "docker");
        assert_eq!(
            args.join(" "),
            "compose run --rm --no-deps --entrypoint composer app require laravel/boost --dev"
        );
        let (program, args) = Mode::Php.composer(&argv);
        assert_eq!(program, "php");
        assert_eq!(
            args[1],
            "passthru('composer ' . \"'require' 'laravel/boost' '--dev'\", $code); exit($code);"
        );
        assert_eq!(Mode::Php.php_args(&["-r".into(), "x".into()]), ["-r", "x"]);
    }
}
