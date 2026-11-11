use crate::runner::Mode;

pub const PROBE: &str = "echo \"\\n@@openidea-runtime@@\", json_encode([PHP_BINARY, PHP_VERSION, PHP_OS_FAMILY, php_ini_loaded_file() ?: '']);";
pub const MARKER: &str = "@@openidea-runtime@@";

pub fn kind_of(binary: &str) -> &'static str {
    let b = binary.replace('\\', "/").to_ascii_lowercase();
    let has = |s: &str| b.contains(s);
    if has("/herd/") || has(".config/herd") || has("herd-lite") || has("/herd") {
        "Herd"
    } else if has("/valet") || has(".config/valet") {
        "Valet"
    } else if has("/opt/homebrew/") || has("/usr/local/cellar/") || has("homebrew") {
        "Homebrew"
    } else if has("laragon") {
        "Laragon"
    } else if has("xampp") {
        "XAMPP"
    } else if has("wamp") {
        "WampServer"
    } else if has("/nix/store/") {
        "Nix"
    } else if has("/scoop/") {
        "Scoop"
    } else if has("chocolatey") {
        "Chocolatey"
    } else if has("/.pvm/") {
        "pvm"
    } else if has("/.phpenv/") || has("/phpenv/") {
        "phpenv"
    } else if has("/usr/bin/php") || has("/usr/local/bin/php") || has("/bin/php") {
        "System"
    } else {
        "Local"
    }
}

pub fn label(mode: &Mode, binary: &str, sail: bool) -> String {
    match mode {
        Mode::Php => kind_of(binary).to_owned(),
        Mode::DockerRun(s) | Mode::DockerExec(s) => {
            if sail && s == "laravel.test" {
                "Sail".into()
            } else {
                format!("Docker Compose · {s}")
            }
        }
    }
}

pub fn advice(mode: &Mode, compose: bool, sail: bool) -> String {
    match mode {
        Mode::Php if sail => "php isn't on the PATH OpenIDEA sees. This project has Laravel Sail: set Run Artisan with to docker in Settings › Laravel OpenIdea to run it in the laravel.test container.".into(),
        Mode::Php if compose => "php isn't on the PATH OpenIDEA sees. This project has a Docker Compose file: set Run Artisan with to docker in Settings › Laravel OpenIdea to run it in a container.".into(),
        Mode::Php => "php isn't on the PATH OpenIDEA sees. Install PHP (or Laravel Herd, which adds php to the PATH) and restart OpenIDEA, so it starts with the new PATH.".into(),
        _ => "docker compose didn't answer. Start Docker, check the service in Settings › Laravel OpenIdea › Docker Compose service, or set Run Artisan with to php.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_php_install() {
        for (path, want) in [
            ("C:\\Users\\dev\\.config\\herd\\bin\\php83\\php.exe", "Herd"),
            (
                "/Users/dev/Library/Application Support/Herd/bin/php83",
                "Herd",
            ),
            ("/opt/homebrew/Cellar/php/8.3.12/bin/php", "Homebrew"),
            ("/Users/dev/.config/valet/bin/php", "Valet"),
            ("C:\\laragon\\bin\\php\\php-8.3\\php.exe", "Laragon"),
            ("C:\\xampp\\php\\php.exe", "XAMPP"),
            ("/usr/bin/php8.3", "System"),
            ("C:\\Users\\dev\\.pvm\\current\\php.exe", "pvm"),
            ("D:\\tools\\php\\php.exe", "Local"),
        ] {
            assert_eq!(kind_of(path), want, "{path}");
        }
        assert_eq!(
            label(&Mode::DockerExec("laravel.test".into()), "", true),
            "Sail"
        );
        assert_eq!(
            label(&Mode::DockerRun("app".into()), "", false),
            "Docker Compose · app"
        );
        assert!(advice(&Mode::Php, true, true).contains("Sail"));
        assert!(advice(&Mode::Php, false, false).contains("Herd"));
    }
}
