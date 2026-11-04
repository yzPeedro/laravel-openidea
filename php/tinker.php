<?php

$__openideaCode = stream_get_contents(STDIN);
$__openideaBase = getcwd();
error_reporting(E_ALL & ~E_DEPRECATED & ~E_USER_DEPRECATED);

$__openideaEmit = static function (array $record): void {
    fwrite(STDOUT, "\n@@openidea@@" . json_encode($record, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR) . "\n");
};

$__openideaStatements = static function (string $code): array {
    $tokens = PhpToken::tokenize('<?php ' . $code);
    array_shift($tokens);
    $out = [];
    $current = '';
    $depth = 0;
    $count = count($tokens);
    $nextWord = static function (int $from) use ($tokens, $count): string {
        for ($j = $from; $j < $count; $j++) {
            if ($tokens[$j]->isIgnorable()) {
                continue;
            }
            return strtolower($tokens[$j]->text);
        }
        return '';
    };
    for ($i = 0; $i < $count; $i++) {
        $token = $tokens[$i];
        $text = $token->text;
        if ($token->is(['{', '(', '[', T_CURLY_OPEN, T_DOLLAR_OPEN_CURLY_BRACES])) {
            $depth++;
        } elseif ($token->is(['}', ')', ']'])) {
            $depth--;
        }
        $current .= $text;
        if ($depth !== 0) {
            continue;
        }
        $ends = $text === ';';
        if ($text === '}' && preg_match('/^\s*(if|foreach|for|while|switch|try|function|class|enum|interface|trait|do|final|abstract|readonly)\b/i', ltrim($current))) {
            $ends = ! in_array($nextWord($i + 1), ['catch', 'finally', 'else', 'elseif', 'while'], true);
        }
        if ($ends) {
            if (trim($current) !== '' && trim($current) !== ';') {
                $out[] = trim($current);
            }
            $current = '';
        }
    }
    if (trim($current) !== '') {
        $out[] = trim($current);
    }
    return $out;
};

try {
    require $__openideaBase . '/vendor/autoload.php';
    $app = require $__openideaBase . '/bootstrap/app.php';
    $app->make(Illuminate\Contracts\Console\Kernel::class)->bootstrap();
} catch (Throwable $e) {
    $__openideaEmit(['kind' => 'boot', 'error' => ['class' => get_class($e), 'message' => $e->getMessage(), 'file' => $e->getFile(), 'line' => $e->getLine()]]);
    exit(1);
}

$__openideaQueries = [];
try {
    Illuminate\Support\Facades\DB::listen(static function ($query) use (&$__openideaQueries) {
        $__openideaQueries[] = ['sql' => $query->sql, 'bindings' => array_map(static fn ($b) => is_scalar($b) || $b === null ? $b : (is_object($b) ? get_class($b) : gettype($b)), $query->bindings), 'ms' => $query->time, 'connection' => $query->connectionName];
    });
} catch (Throwable) {
}

$__openideaShell = null;
$__openideaPresenter = null;
$__openideaBuffer = null;
if (class_exists(Psy\Shell::class)) {
    $tmp = sys_get_temp_dir();
    $config = new Psy\Configuration([
        'configDir' => $tmp,
        'dataDir' => $tmp,
        'runtimeDir' => $tmp,
        'updateCheck' => 'never',
        'usePcntl' => false,
        'startupMessage' => '',
        'colorMode' => defined('Psy\\Configuration::COLOR_MODE_FORCED') ? Psy\Configuration::COLOR_MODE_FORCED : 'forced',
    ]);
    $casters = [];
    if (class_exists(Laravel\Tinker\TinkerCaster::class)) {
        foreach ([
            'Illuminate\Support\Collection' => 'castCollection',
            'Illuminate\Database\Eloquent\Model' => 'castModel',
            'Illuminate\Foundation\Application' => 'castApplication',
            'Illuminate\Support\Stringable' => 'castStringable',
            'Illuminate\Support\HtmlString' => 'castHtmlString',
            'Illuminate\Process\ProcessResult' => 'castProcessResult',
        ] as $class => $method) {
            if (class_exists($class) && method_exists(Laravel\Tinker\TinkerCaster::class, $method)) {
                $casters[$class] = 'Laravel\Tinker\TinkerCaster::' . $method;
            }
        }
    }
    $config->getPresenter()->addCasters($casters);
    $__openideaShell = new Psy\Shell($config);
    $__openideaBuffer = new Symfony\Component\Console\Output\BufferedOutput();
    if (method_exists($__openideaShell, 'setOutput')) {
        $__openideaShell->setOutput($__openideaBuffer);
    }
    $__openideaShell->setScopeVariables(['app' => $app]);
    $__openideaPresenter = $config->getPresenter();
    if (class_exists(Laravel\Tinker\ClassAliasAutoloader::class) && is_file($__openideaBase . '/vendor/composer/autoload_classmap.php')) {
        try {
            Laravel\Tinker\ClassAliasAutoloader::register($__openideaShell, $__openideaBase . '/vendor/composer/autoload_classmap.php', config('tinker.alias', []), config('tinker.dont_alias', []));
        } catch (Throwable) {
        }
    }
}

spl_autoload_register(static function (string $class) use ($__openideaBase): void {
    if (str_contains($class, '\\')) {
        return;
    }
    foreach (['App\\Models\\', 'App\\'] as $namespace) {
        if (is_file($__openideaBase . '/app/' . str_replace('\\', '/', substr($namespace, 4)) . $class . '.php') && class_exists($namespace . $class)) {
            class_alias($namespace . $class, $class);
            return;
        }
    }
});

if (class_exists(Symfony\Component\VarDumper\VarDumper::class)) {
    Symfony\Component\VarDumper\VarDumper::setHandler(static function ($value) use (&$__openideaPresenter, $__openideaEmit) {
        $frame = null;
        foreach (debug_backtrace(DEBUG_BACKTRACE_IGNORE_ARGS, 12) as $f) {
            if (isset($f['file']) && ! str_contains($f['file'], '/vendor/') && ! str_contains($f['file'], 'eval()')) {
                $frame = $f;
                break;
            }
        }
        $text = $__openideaPresenter ? $__openideaPresenter->present($value) : print_r($value, true);
        $__openideaEmit(['kind' => 'dump', 'value' => $text, 'file' => $frame['file'] ?? null, 'line' => $frame['line'] ?? null]);
    });
}

foreach ($__openideaStatements($__openideaCode) as $__openideaIndex => $__openideaStatement) {
    $__openideaQueries = [];
    $started = microtime(true);
    ob_start();
    $error = null;
    $value = null;
    $hasValue = false;
    try {
        if ($__openideaShell) {
            $value = $__openideaShell->execute($__openideaStatement, true);
        } else {
            $value = eval('return ' . rtrim($__openideaStatement, ';') . ';');
        }
        $hasValue = true;
    } catch (Throwable $e) {
        $frames = [];
        foreach (array_slice($e->getTrace(), 0, 12) as $f) {
            if (isset($f['file'])) {
                $frames[] = ['file' => $f['file'], 'line' => $f['line'] ?? 0, 'call' => ($f['class'] ?? '') . ($f['type'] ?? '') . ($f['function'] ?? '')];
            }
        }
        $error = ['class' => get_class($e), 'message' => $e->getMessage(), 'file' => $e->getFile(), 'line' => $e->getLine(), 'trace' => $frames];
    }
    $output = ob_get_clean() . ($__openideaBuffer ? $__openideaBuffer->fetch() : '');
    if ($hasValue && is_object($value) && $value instanceof Psy\CodeCleaner\NoReturnValue) {
        $hasValue = false;
    }
    if ($error && str_contains($error['file'], "eval()'d code")) {
        $error['file'] = null;
    }
    $silent = (bool) preg_match('/^\s*(echo|print|unset|use|namespace|function|class|enum|interface|trait|if|foreach|for|while|switch|try)\b/i', $__openideaStatement)
        || (bool) preg_match('/;\s*;\s*$/', $__openideaStatement);
    $presented = null;
    if ($hasValue && ! $silent) {
        try {
            $presented = $__openideaPresenter ? $__openideaPresenter->present($value) : var_export($value, true);
        } catch (Throwable $e) {
            $presented = get_debug_type($value);
        }
    }
    $__openideaEmit([
        'kind' => 'statement',
        'index' => $__openideaIndex,
        'code' => $__openideaStatement,
        'output' => $output,
        'result' => $presented,
        'type' => $hasValue ? get_debug_type($value) : null,
        'error' => $error,
        'ms' => round((microtime(true) - $started) * 1000, 2),
        'queries' => $__openideaQueries,
    ]);
    if ($error) {
        break;
    }
}
$__openideaEmit(['kind' => 'done', 'memory' => memory_get_peak_usage(true), 'php' => PHP_VERSION, 'laravel' => $app->version(), 'env' => $app->environment()]);
