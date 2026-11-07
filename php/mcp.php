<?php

$base = getcwd();
if (! is_file($base . '/artisan')) {
    $base = dirname(__DIR__, 2);
}
chdir($base);
error_reporting(E_ALL & ~E_DEPRECATED & ~E_USER_DEPRECATED);
ini_set('display_errors', 'stderr');

$send = static function (array $message): void {
    fwrite(STDOUT, json_encode($message, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR) . "\n");
    fflush(STDOUT);
};

if (($argv[1] ?? '') !== '--worker') {
    $accessFile = $base . '/storage/openidea/mcp-access.json';
    $stamp = static function () use ($accessFile): string {
        clearstatcache();
        return (is_file($accessFile) ? (string) md5_file($accessFile) : '') . '|' . (string) @md5_file(__FILE__);
    };
    $seen = null;
    while (($line = fgets(STDIN)) !== false) {
        $line = trim($line);
        if ($line === '') {
            continue;
        }
        $msg = json_decode($line, true);
        if (! is_array($msg)) {
            $send(['jsonrpc' => '2.0', 'id' => null, 'error' => ['code' => -32700, 'message' => 'Parse error']]);
            continue;
        }
        $id = $msg['id'] ?? null;
        $method = (string) ($msg['method'] ?? '');
        if ($id === null) {
            continue;
        }
        if ($method === 'initialize') {
            $seen = $stamp();
            $send(['jsonrpc' => '2.0', 'id' => $id, 'result' => [
                'protocolVersion' => $msg['params']['protocolVersion'] ?? '2025-06-18',
                'capabilities' => ['tools' => ['listChanged' => true]],
                'serverInfo' => ['name' => 'laravel-openidea', 'version' => '2.0.0'],
                'instructions' => 'Live view and control of this Laravel application from Laravel OpenIDEA: routes, models, classes, events, bindings, schedule, queues, config, schema, translations, tests, logs and the Live Audit recorder, plus Artisan, PHPStan, Pint, tests and Tinker as the access level allows. Every call boots the application again, so answers follow the code as it changes. Call app_overview first and access_level to see what the user allowed.',
            ]]);
            continue;
        }
        if ($method === 'ping') {
            $send(['jsonrpc' => '2.0', 'id' => $id, 'result' => new stdClass()]);
            continue;
        }
        $now = $stamp();
        if ($seen !== null && $now !== $seen) {
            $send(['jsonrpc' => '2.0', 'method' => 'notifications/tools/list_changed']);
        }
        $seen = $now;
        $proc = proc_open([PHP_BINARY, __FILE__, '--worker'], [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']], $pipes, $base);
        if (! is_resource($proc)) {
            $send(['jsonrpc' => '2.0', 'id' => $id, 'error' => ['code' => -32603, 'message' => 'Could not start PHP for this call']]);
            continue;
        }
        fwrite($pipes[0], $line);
        fclose($pipes[0]);
        stream_set_blocking($pipes[1], false);
        stream_set_blocking($pipes[2], false);
        $out = '';
        $err = '';
        $code = null;
        while (true) {
            $out .= (string) stream_get_contents($pipes[1]);
            $err .= (string) stream_get_contents($pipes[2]);
            $status = proc_get_status($proc);
            if (! $status['running']) {
                $code = $status['exitcode'];
                break;
            }
            usleep(5000);
        }
        $out .= (string) stream_get_contents($pipes[1]);
        $err .= (string) stream_get_contents($pipes[2]);
        fclose($pipes[1]);
        fclose($pipes[2]);
        proc_close($proc);
        $reply = null;
        foreach (array_reverse(preg_split('/\R/', trim($out)) ?: []) as $candidate) {
            $decoded = json_decode($candidate, true);
            if (is_array($decoded) && ($decoded['jsonrpc'] ?? null) === '2.0') {
                $reply = $decoded;
                break;
            }
        }
        $detail = trim($err) !== '' ? trim($err) : 'it exited with code ' . $code;
        $send($reply ?? ['jsonrpc' => '2.0', 'id' => $id, 'error' => ['code' => -32603, 'message' => 'The call stopped before answering: ' . mb_substr($detail, -2000)]]);
    }
    exit(0);
}

$levels = ['read' => 0, 'write' => 1, 'full' => 2];
$level = (static function () use ($base, $levels): string {
    $file = $base . '/storage/openidea/mcp-access.json';
    $data = is_file($file) ? json_decode((string) file_get_contents($file), true) : null;
    $value = is_array($data) ? (string) ($data['level'] ?? 'read') : 'read';
    return isset($levels[$value]) ? $value : 'read';
})();
$allows = static fn (string $needed): bool => $levels[$needed] <= $levels[$level];
$denied = static fn (string $needed, string $what): RuntimeException => new RuntimeException("$what needs $needed access, and the user allowed $level for this project. Only the user can change it, in OpenIDEA: Settings › Plugins › Laravel OpenIDEA › MCP access.");


const OPENIDEA_SECRET = '/pass(word)?|secret|token|api[_-]?key|authorization|cookie|credential|private|signature|dsn|_key$/i';

$app = null;
$boot = static function () use (&$app, $base) {
    if ($app) {
        return $app;
    }
    ob_start();
    try {
        require $base . '/vendor/autoload.php';
        $app = require $base . '/bootstrap/app.php';
        $app->make(Illuminate\Contracts\Console\Kernel::class)->bootstrap();
    } finally {
        ob_end_clean();
    }
    return $app;
};

$mask = static function ($value, ?string $key = null, int $depth = 0) use (&$mask) {
    if ($key !== null && preg_match(OPENIDEA_SECRET, $key)) {
        return $value === null || $value === '' ? $value : '•••';
    }
    if (is_array($value)) {
        if ($depth > 6) {
            return '[…]';
        }
        $out = [];
        foreach ($value as $k => $v) {
            $out[$k] = $mask($v, is_string($k) ? $k : null, $depth + 1);
        }
        return $out;
    }
    if (is_object($value)) {
        return $value instanceof Closure ? '{Closure}' : '{' . get_class($value) . '}';
    }
    return $value;
};

$like = static fn (?string $filter, string ...$values): bool => $filter === null || $filter === '' || array_filter($values, static fn ($v) => stripos($v, $filter) !== false) !== [];

$label = static function ($listener): string {
    if (is_string($listener)) {
        return $listener;
    }
    if (is_array($listener) && count($listener) === 2) {
        return (is_object($listener[0]) ? get_class($listener[0]) : (string) $listener[0]) . '@' . $listener[1];
    }
    if ($listener instanceof Closure) {
        try {
            $r = new ReflectionFunction($listener);
            return 'Closure ' . str_replace(getcwd() . '/', '', (string) $r->getFileName()) . ':' . $r->getStartLine();
        } catch (Throwable) {
            return 'Closure';
        }
    }
    return get_debug_type($listener);
};

$appClasses = static function (callable $keep, array $dirs = ['app']) use ($base): array {
    $map = is_file($base . '/vendor/composer/autoload_classmap.php') ? require $base . '/vendor/composer/autoload_classmap.php' : [];
    $under = static function (string $file) use ($base, $dirs): bool {
        foreach ($dirs as $dir) {
            if (str_starts_with((string) realpath($file), $base . '/' . $dir . '/') || str_starts_with($file, $base . '/' . $dir . '/')) {
                return true;
            }
        }
        return false;
    };
    $classes = [];
    foreach ($map as $class => $file) {
        if ($under($file)) {
            $classes[$class] = true;
        }
    }
    foreach ($dirs as $dir) {
        if (! is_dir($base . '/' . $dir)) {
            continue;
        }
        foreach (new RecursiveIteratorIterator(new RecursiveDirectoryIterator($base . '/' . $dir, FilesystemIterator::SKIP_DOTS)) as $f) {
            if ($f->getExtension() !== 'php' || str_ends_with($f->getFilename(), '.blade.php')) {
                continue;
            }
            $code = (string) file_get_contents($f->getPathname());
            if (! preg_match('/^\s*(?:final\s+|abstract\s+|readonly\s+)*(?:class|interface|trait|enum)\s+(\w+)/m', $code, $cl)) {
                continue;
            }
            $classes[(preg_match('/^namespace\s+([^;]+);/m', $code, $ns) ? $ns[1] . '\\' : '') . $cl[1]] = true;
        }
    }
    $out = [];
    foreach (array_keys($classes) as $class) {
        try {
            if (! class_exists($class) && ! interface_exists($class) && ! trait_exists($class) && ! enum_exists($class)) {
                continue;
            }
            $r = new ReflectionClass($class);
            if ($keep($r)) {
                $out[] = $r;
            }
        } catch (Throwable) {
        }
    }
    usort($out, static fn ($a, $b) => strcmp($a->getName(), $b->getName()));
    return $out;
};

$rel = static fn (?string $file): ?string => $file ? ltrim(str_replace([$base, '/var/www/html'], '', $file), '/') : null;

$tools = [];
$tool = static function (string $name, string $level, string $description, array $properties, callable $run) use (&$tools) {
    $tools[$name] = ['level' => $level, 'description' => $description, 'properties' => $properties, 'run' => $run];
};
$str = static fn (string $d) => ['type' => 'string', 'description' => $d];
$int = static fn (string $d) => ['type' => 'integer', 'description' => $d];

$tool('app_overview', 'read', 'Laravel OpenIDEA hub overview: Laravel and PHP versions, environment, drivers (database, cache, queue, session, mail, broadcasting), first-party and popular packages installed, and counts of routes, models, events, listeners, commands and scheduled tasks. Start here.', [], function () use ($boot, $base) {
    $app = $boot();
    $lock = is_file($base . '/composer.lock') ? json_decode((string) file_get_contents($base . '/composer.lock'), true) : [];
    $packages = [];
    foreach (array_merge($lock['packages'] ?? [], $lock['packages-dev'] ?? []) as $p) {
        if (preg_match('#^(laravel|livewire|inertiajs|spatie|filament|barryvdh|itsgoingd|laradumps|pestphp|larastan|nunomaduro|tightenco)/#', $p['name'])) {
            $packages[$p['name']] = ltrim($p['version'], 'v');
        }
    }
    $events = $app['events']->getRawListeners();
    return [
        'laravel' => $app->version(),
        'php' => PHP_VERSION,
        'environment' => $app->environment(),
        'debug' => (bool) config('app.debug'),
        'url' => config('app.url'),
        'timezone' => config('app.timezone'),
        'locale' => config('app.locale'),
        'drivers' => [
            'database' => config('database.default'),
            'cache' => config('cache.default'),
            'queue' => config('queue.default'),
            'session' => config('session.driver'),
            'mail' => config('mail.default'),
            'broadcasting' => config('broadcasting.default'),
            'filesystem' => config('filesystems.default'),
            'log' => config('logging.default'),
        ],
        'counts' => [
            'routes' => count(app('router')->getRoutes()->getRoutes()),
            'events_with_listeners' => count($events),
            'listeners' => array_sum(array_map('count', $events)),
            'artisan_commands' => count(Illuminate\Support\Facades\Artisan::all()),
            'scheduled_tasks' => count(app(Illuminate\Console\Scheduling\Schedule::class)->events()),
        ],
        'packages' => $packages,
        'hub' => 'The Laravel OpenIDEA hub (bottom tool window in OpenIDEA) shows the same data with filters: routes, controllers, form requests, API resources, middleware, models, scopes, observers, casts, enums, database, factories, views, components, translations, classes, providers, bindings, rules, macros, config, env, drivers, auth, queues, schedule, events, listeners, mail, tests, live audit, logs, profiles, debug calls, Artisan and Tinker.',
    ];
});

$tool('list_routes', 'read', 'Routes with method, URI, name, action (Controller@method), middleware and the controller file. Filter matches URI, name or action.', ['filter' => $str('Text to match'), 'method' => $str('GET, POST, PUT, PATCH or DELETE')], function (array $a) use ($boot, $like, $rel) {
    $boot();
    $out = [];
    foreach (app('router')->getRoutes()->getRoutes() as $route) {
        $methods = array_values(array_diff($route->methods(), ['HEAD']));
        if (! empty($a['method']) && ! in_array(strtoupper($a['method']), $methods, true)) {
            continue;
        }
        $action = $route->getActionName();
        if (! $like($a['filter'] ?? null, $route->uri(), (string) $route->getName(), $action)) {
            continue;
        }
        $file = null;
        try {
            if (str_contains($action, '@')) {
                [$c, $m] = explode('@', $action);
                $rm = new ReflectionMethod($c, $m);
                $file = $rel($rm->getFileName()) . ':' . $rm->getStartLine();
            }
        } catch (Throwable) {
        }
        $out[] = ['methods' => implode('|', $methods), 'uri' => '/' . ltrim($route->uri(), '/'), 'name' => $route->getName(), 'action' => $action, 'middleware' => $route->gatherMiddleware(), 'file' => $file];
    }
    return $out;
});

$tool('list_events_and_listeners', 'read', 'Every event the application listens to and its listeners, as registered at runtime (event discovery, Event::listen, subscribers, model observers as eloquent.* events, queued listeners).', ['filter' => $str('Text to match in the event or listener')], function (array $a) use ($boot, $like, $label) {
    $app = $boot();
    $out = [];
    foreach ($app['events']->getRawListeners() as $event => $listeners) {
        $names = array_map($label, $listeners);
        if (! $like($a['filter'] ?? null, $event, implode(' ', $names))) {
            continue;
        }
        $out[] = ['event' => $event, 'listeners' => $names];
    }
    usort($out, static fn ($x, $y) => strcmp($x['event'], $y['event']));
    return $out;
});

$tool('list_models', 'read', 'Eloquent models with table, connection, primary key, fillable, guarded, hidden, casts, relations (method → type and related model), local scopes, global scopes and observers.', ['filter' => $str('Model name to match')], function (array $a) use ($boot, $appClasses, $like, $rel, $label) {
    $app = $boot();
    $models = $appClasses(static fn (ReflectionClass $r) => $r->isSubclassOf(Illuminate\Database\Eloquent\Model::class) && ! $r->isAbstract());
    $raw = $app['events']->getRawListeners();
    $out = [];
    foreach ($models as $r) {
        if (! $like($a['filter'] ?? null, $r->getName())) {
            continue;
        }
        try {
            $m = new ($r->getName())();
        } catch (Throwable) {
            continue;
        }
        $relations = [];
        $scopes = [];
        foreach ($r->getMethods(ReflectionMethod::IS_PUBLIC) as $method) {
            if ($method->class !== $r->getName() || $method->getNumberOfRequiredParameters() > 0) {
                if (str_starts_with($method->name, 'scope') && $method->class === $r->getName()) {
                    $scopes[] = lcfirst(substr($method->name, 5));
                }
                continue;
            }
            $type = $method->getReturnType();
            $typeName = $type instanceof ReflectionNamedType ? $type->getName() : null;
            if ($typeName && is_subclass_of($typeName, Illuminate\Database\Eloquent\Relations\Relation::class)) {
                $related = null;
                try {
                    $related = get_class($m->{$method->name}()->getRelated());
                } catch (Throwable) {
                }
                $relations[$method->name] = class_basename($typeName) . ($related ? ' → ' . $related : '');
            }
        }
        foreach ($r->getMethods() as $method) {
            if ($method->class === $r->getName() && $method->getAttributes('Illuminate\Database\Eloquent\Attributes\Scope') !== []) {
                $scopes[] = $method->name;
            }
        }
        $observers = [];
        foreach ($raw as $event => $listeners) {
            if (str_starts_with($event, 'eloquent.') && str_ends_with($event, ': ' . $r->getName())) {
                foreach ($listeners as $l) {
                    $observers[] = substr($event, 9, strpos($event, ':') - 9) . ' → ' . $label($l);
                }
            }
        }
        $out[] = [
            'model' => $r->getName(),
            'file' => $rel($r->getFileName()),
            'table' => $m->getTable(),
            'connection' => $m->getConnectionName() ?? config('database.default'),
            'key' => $m->getKeyName(),
            'fillable' => $m->getFillable(),
            'guarded' => $m->getGuarded(),
            'hidden' => $m->getHidden(),
            'casts' => $m->getCasts(),
            'relations' => $relations,
            'scopes' => $scopes,
            'global_scopes' => array_map(static fn ($s) => is_object($s) ? get_class($s) : (string) $s, array_keys($m->getGlobalScopes())),
            'observers' => $observers,
        ];
    }
    return $out;
});

$tool('list_container_bindings', 'read', 'Service container bindings: abstract, whether shared (singleton), and the concrete class or closure location. Filter matches the abstract.', ['filter' => $str('Text to match')], function (array $a) use ($boot, $like, $label) {
    $app = $boot();
    $out = [];
    foreach ($app->getBindings() as $abstract => $binding) {
        if (! $like($a['filter'] ?? null, $abstract)) {
            continue;
        }
        $concrete = $binding['concrete'] ?? null;
        $text = $label($concrete);
        if ($concrete instanceof Closure) {
            try {
                $vars = (new ReflectionFunction($concrete))->getStaticVariables();
                if (isset($vars['concrete']) && is_string($vars['concrete'])) {
                    $text = $vars['concrete'];
                }
            } catch (Throwable) {
            }
        }
        $out[] = ['abstract' => $abstract, 'shared' => (bool) ($binding['shared'] ?? false), 'concrete' => $text];
    }
    return $out;
});

$tool('list_schedule', 'read', 'Scheduled tasks with cron expression, command or job, description, next run, timezone, overlapping and one-server flags.', [], function () use ($boot) {
    $boot();
    $out = [];
    foreach (app(Illuminate\Console\Scheduling\Schedule::class)->events() as $event) {
        $next = null;
        try {
            $next = $event->nextRunDate()->format('Y-m-d H:i:s T');
        } catch (Throwable) {
        }
        $out[] = ['expression' => $event->expression, 'command' => $event->command ? preg_replace("/^.*artisan'?\s+/", 'artisan ', $event->command) : null, 'description' => $event->description, 'next' => $next, 'timezone' => (string) ($event->timezone ?? config('app.timezone')), 'without_overlapping' => $event->withoutOverlapping, 'one_server' => $event->onOneServer];
    }
    return $out;
});

$tool('list_artisan_commands', 'read', 'Artisan commands with description and usage. Filter matches the name or description.', ['filter' => $str('Text to match')], function (array $a) use ($boot, $like) {
    $boot();
    $out = [];
    foreach (Illuminate\Support\Facades\Artisan::all() as $name => $command) {
        if (! $like($a['filter'] ?? null, $name, (string) $command->getDescription())) {
            continue;
        }
        $out[] = ['name' => $name, 'description' => $command->getDescription(), 'usage' => $command->getSynopsis(true), 'class' => get_class($command)];
    }
    return $out;
});

$tool('get_config', 'read', 'Configuration values for a key or a whole file (for example "database.connections.mysql" or "queue"). Secrets are masked.', ['key' => $str('Config key or file name')], function (array $a) use ($boot, $mask) {
    $boot();
    $key = (string) ($a['key'] ?? 'app');
    return ['key' => $key, 'value' => $mask(config($key), str_contains($key, '.') ? substr($key, strrpos($key, '.') + 1) : null)];
});

$tool('database_schema', 'read', 'Database tables, or the columns, indexes and foreign keys of one table, from the live connection.', ['table' => $str('Table name; leave empty to list tables'), 'connection' => $str('Connection name')], function (array $a) use ($boot) {
    $boot();
    $schema = Illuminate\Support\Facades\Schema::connection($a['connection'] ?? null);
    if (empty($a['table'])) {
        return array_map(static fn ($t) => ['name' => $t['name'], 'rows' => $t['rows'] ?? null, 'size' => $t['size'] ?? null], $schema->getTables());
    }
    return ['table' => $a['table'], 'columns' => $schema->getColumns($a['table']), 'indexes' => $schema->getIndexes($a['table']), 'foreign_keys' => $schema->getForeignKeys($a['table'])];
});

$tool('audit_recent', 'read', 'Recent records from the Live Audit recorder of the hub (requests with input and timing, SQL with bindings and the calling line, events with payloads and listeners, jobs, cache, mail, notifications, HTTP client, gate checks, logs, exceptions, dumps), plus insights: N+1 query patterns, slow queries, exceptions and failed jobs. Turn the recorder on in the hub first.', ['types' => $str('Comma separated types, for example "request,query,exception"'), 'limit' => $int('Records to return, newest last (default 200)'), 'filter' => $str('Text to match')], function (array $a) use ($base) {
    $file = $base . '/storage/logs/openidea-audit.jsonl';
    if (! is_file($file)) {
        return ['recorder' => 'off', 'hint' => 'Turn the recorder on in Laravel OpenIDEA › Live Audit, then reproduce the behaviour.'];
    }
    $types = array_filter(array_map('trim', explode(',', (string) ($a['types'] ?? ''))));
    $limit = max(1, min(2000, (int) ($a['limit'] ?? 200)));
    $size = filesize($file);
    $h = fopen($file, 'r');
    fseek($h, max(0, $size - 4_000_000));
    $lines = explode("\n", (string) stream_get_contents($h));
    fclose($h);
    $records = [];
    foreach ($lines as $line) {
        $r = json_decode($line, true);
        if (! is_array($r)) {
            continue;
        }
        $records[] = $r;
    }
    $insights = [];
    $groups = [];
    foreach ($records as $r) {
        if (($r['type'] ?? '') === 'query') {
            $k = $r['scope'] . '|' . preg_replace('/\s+/', ' ', $r['sql']) . '|' . ($r['file'] ?? '') . ':' . ($r['line'] ?? '');
            $groups[$k][] = $r;
            if (($r['ms'] ?? 0) >= 100) {
                $insights[] = ['kind' => 'slow query', 'ms' => $r['ms'], 'sql' => $r['sql'], 'at' => ($r['file'] ?? '') . ':' . ($r['line'] ?? '')];
            }
        }
        if (($r['type'] ?? '') === 'exception') {
            $insights[] = ['kind' => 'exception', 'class' => $r['class'], 'message' => $r['message'], 'at' => ($r['file'] ?? '') . ':' . ($r['line'] ?? '')];
        }
        if (($r['type'] ?? '') === 'job' && ($r['status'] ?? '') === 'failed') {
            $insights[] = ['kind' => 'failed job', 'job' => $r['job'], 'error' => $r['error'] ?? null];
        }
    }
    foreach ($groups as $qs) {
        if (count($qs) >= 5) {
            $insights[] = ['kind' => 'possible N+1', 'count' => count($qs), 'sql' => $qs[0]['sql'], 'at' => ($qs[0]['file'] ?? '') . ':' . ($qs[0]['line'] ?? '')];
        }
    }
    $filtered = array_values(array_filter($records, static fn ($r) => ($types === [] || in_array($r['type'] ?? '', $types, true)) && (empty($a['filter']) || stripos(json_encode($r), (string) $a['filter']) !== false)));
    return ['insights' => array_slice($insights, -50), 'records' => array_slice($filtered, -$limit)];
});

$tool('read_logs', 'read', 'The last entries of the Laravel log (storage/logs/laravel.log or the newest daily file), optionally only one level.', ['lines' => $int('Entries to return (default 30)'), 'level' => $str('error, warning, info, debug…')], function (array $a) use ($base) {
    $files = glob($base . '/storage/logs/*.log') ?: [];
    usort($files, static fn ($x, $y) => filemtime($y) <=> filemtime($x));
    if ($files === []) {
        return [];
    }
    $h = fopen($files[0], 'r');
    fseek($h, max(0, filesize($files[0]) - 2_000_000));
    $text = (string) stream_get_contents($h);
    fclose($h);
    preg_match_all('/^\[(\d{4}-\d\d-\d\d[ T][\d:.+\-]+)\] (\w+)\.(\w+): (.*?)(?=^\[\d{4}-\d\d-\d\d|\z)/ms', $text, $m, PREG_SET_ORDER);
    $out = [];
    foreach ($m as $e) {
        if (! empty($a['level']) && strcasecmp($e[3], $a['level']) !== 0) {
            continue;
        }
        $out[] = ['time' => $e[1], 'level' => strtolower($e[3]), 'message' => mb_substr(trim($e[4]), 0, 3000)];
    }
    return ['file' => basename($files[0]), 'entries' => array_slice($out, -max(1, min(500, (int) ($a['lines'] ?? 30))))];
});

$tool('list_queues', 'read', 'Queue connections, the default queue, queued job classes of the app with tries, timeout and queue, and the failed jobs count when the failer is configured.', [], function () use ($boot, $appClasses, $rel) {
    $boot();
    $jobs = [];
    foreach ($appClasses(static fn (ReflectionClass $r) => ! $r->isAbstract() && $r->implementsInterface(Illuminate\Contracts\Queue\ShouldQueue::class)) as $r) {
        $d = $r->getDefaultProperties();
        $jobs[] = ['class' => $r->getName(), 'file' => $rel($r->getFileName()), 'queue' => $d['queue'] ?? null, 'tries' => $d['tries'] ?? null, 'timeout' => $d['timeout'] ?? null];
    }
    $failed = null;
    try {
        $failed = count(app('queue.failer')->all());
    } catch (Throwable) {
    }
    return ['default' => config('queue.default'), 'connections' => array_keys(config('queue.connections', [])), 'jobs' => $jobs, 'failed_jobs' => $failed];
});

$tool('missing_translations', 'read', 'Translation keys present in one locale but missing in another (PHP files in lang/ and JSON files).', ['locale' => $str('Only report keys missing in this locale')], function (array $a) use ($base) {
    $dir = is_dir($base . '/lang') ? $base . '/lang' : $base . '/resources/lang';
    $keys = [];
    foreach (glob($dir . '/*.json') ?: [] as $f) {
        $keys[basename($f, '.json')] = array_keys(json_decode((string) file_get_contents($f), true) ?: []);
    }
    foreach (glob($dir . '/*', GLOB_ONLYDIR) ?: [] as $d) {
        $locale = basename($d);
        if ($locale === 'vendor') {
            continue;
        }
        foreach (glob($d . '/*.php') ?: [] as $f) {
            $flat = static function ($arr, $prefix) use (&$flat) {
                $o = [];
                foreach ((array) $arr as $k => $v) {
                    is_array($v) ? array_push($o, ...$flat($v, $prefix . $k . '.')) : $o[] = $prefix . $k;
                }
                return $o;
            };
            $keys[$locale] = array_merge($keys[$locale] ?? [], $flat(require $f, basename($f, '.php') . '.'));
        }
    }
    $all = array_unique(array_merge(...array_values($keys ?: [[]])));
    $out = [];
    foreach ($keys as $locale => $list) {
        if (! empty($a['locale']) && $a['locale'] !== $locale) {
            continue;
        }
        $missing = array_values(array_diff($all, $list));
        $out[$locale] = ['missing' => count($missing), 'keys' => array_slice($missing, 0, 300)];
    }
    return $out;
});

$tool('run_tinker', 'full', 'Runs PHP inside the booted application (like php artisan tinker) and returns each statement result, echo output and SQL queries. Needs full access. Use for quick checks in the local environment; avoid writes unless the user asks.', ['code' => $str('PHP statements')], function (array $a) use ($boot) {
    $app = $boot();
    $queries = [];
    Illuminate\Support\Facades\DB::listen(static function ($q) use (&$queries) {
        $queries[] = ['sql' => $q->sql, 'bindings' => $q->bindings, 'ms' => $q->time];
    });
    $out = [];
    $tmp = sys_get_temp_dir();
    $shell = class_exists(Psy\Shell::class) ? new Psy\Shell(new Psy\Configuration(['configDir' => $tmp, 'dataDir' => $tmp, 'runtimeDir' => $tmp, 'updateCheck' => 'never', 'usePcntl' => false])) : null;
    $buffer = new Symfony\Component\Console\Output\BufferedOutput();
    if ($shell) {
        $shell->setOutput($buffer);
    }
    ob_start();
    try {
        $value = $shell ? $shell->execute((string) $a['code'], true) : eval((string) $a['code']);
        $printable = $value;
        if (is_object($value)) {
            $printable = method_exists($value, 'toArray') ? $value->toArray() : (method_exists($value, '__toString') ? (string) $value : get_class($value));
        }
        $out['result'] = $printable;
        $out['type'] = get_debug_type($value);
    } catch (Throwable $e) {
        $out['error'] = get_class($e) . ': ' . $e->getMessage();
    }
    $out['output'] = ob_get_clean() . $buffer->fetch();
    $out['queries'] = $queries;
    return $out;
});

$tool('hub_sections', 'read', 'What the Laravel OpenIDEA hub offers and where, so you can point the user to it.', [], fn () => [
    'Develop' => ['Artisan: every command as a form, run in the terminal or with output in place', 'Tinker: editor tabs, Ctrl+Enter runs, Tinker-style output'],
    'HTTP' => ['Routes (filters by method, prefix, controller; click opens the method)', 'Controllers', 'Form Requests', 'API Resources', 'Middleware'],
    'Data' => ['Models', 'Scopes', 'Observers', 'Casts', 'Enums', 'Database', 'Factories & Seeders'],
    'Views' => ['Views', 'Components', 'Frontend (Vite, Inertia)', 'Translations (filter by locale and group)'],
    'App' => ['All Classes', 'Service Providers', 'Container Bindings', 'Validation Rules', 'Macros'],
    'Configure' => ['Config', 'Environment', 'Drivers & Connections', 'Auth & Policies'],
    'Run' => ['Queues & Workers', 'Scheduling', 'Events', 'Listeners', 'Mail & Notifications', 'Tests'],
    'Observe' => ['Live Audit (recorder)', 'Logs (live, level filters)', 'Profiles (Clockwork, Debugbar)', 'Debug Calls', 'Debug Tools'],
    'Project' => ['Ecosystem (Sanctum, Wayfinder, Horizon, Reverb, Octane, starter kits…)', 'AI Agents (this MCP server, Laravel Boost)'],
]);

$list = static fn (string $d) => ['type' => 'array', 'items' => ['type' => 'string'], 'description' => $d];
$bool = static fn (string $d) => ['type' => 'boolean', 'description' => $d];

$run = static function (array $argv, int $seconds = 120, ?string $stdin = null, int $limit = 60000) use ($base): array {
    $proc = proc_open($argv, [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']], $pipes, $base);
    if (! is_resource($proc)) {
        throw new RuntimeException('Could not start ' . implode(' ', array_slice($argv, 1, 2)));
    }
    fwrite($pipes[0], (string) $stdin);
    fclose($pipes[0]);
    stream_set_blocking($pipes[1], false);
    stream_set_blocking($pipes[2], false);
    $out = '';
    $err = '';
    $code = null;
    $timeout = false;
    $deadline = microtime(true) + $seconds;
    while (true) {
        $out .= (string) stream_get_contents($pipes[1]);
        $err .= (string) stream_get_contents($pipes[2]);
        $status = proc_get_status($proc);
        if (! $status['running']) {
            $code = $status['exitcode'];
            break;
        }
        if (microtime(true) >= $deadline) {
            proc_terminate($proc, 9);
            $timeout = true;
            break;
        }
        usleep(20000);
    }
    $out .= (string) stream_get_contents($pipes[1]);
    $err .= (string) stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    proc_close($proc);
    $clip = static fn (string $s): string => strlen($s) > $limit ? '…' . substr($s, -$limit) : $s;
    return ['exit_code' => $code, 'timed_out' => $timeout, 'output' => $clip($out), 'errors' => $clip($err)];
};

$inside = static function ($path) use ($base): string {
    $path = str_replace('\\', '/', trim((string) $path));
    if (str_starts_with($path, $base . '/')) {
        $path = substr($path, strlen($base) + 1);
    }
    $path = rtrim((string) preg_replace('#^(\./)+#', '', $path), '/');
    if ($path === '' || $path === '.' || str_starts_with($path, '/') || preg_match('#(^|/)\.\.(/|$)#', $path)) {
        throw new InvalidArgumentException("Use a path relative to the project root: $path");
    }
    foreach (['vendor', 'storage', 'bootstrap/cache', 'node_modules', '.git'] as $dir) {
        if ($path === $dir || str_starts_with($path, $dir . '/')) {
            throw new InvalidArgumentException("$path is outside what this tool may touch");
        }
    }
    if (! file_exists($base . '/' . $path)) {
        throw new InvalidArgumentException("$path does not exist");
    }
    return $path;
};

$artisanLevel = static function (string $command): string {
    $read = ['about', 'list', 'help', 'env', 'inspire', 'route:list', 'migrate:status', 'schedule:list', 'event:list', 'model:show', 'db:show', 'db:table', 'db:monitor', 'channel:list', 'queue:failed', 'queue:monitor'];
    $write = ['migrate', 'db:seed', 'cache:clear', 'config:cache', 'config:clear', 'route:cache', 'route:clear', 'view:cache', 'view:clear', 'event:cache', 'event:clear', 'optimize', 'optimize:clear', 'storage:link', 'queue:retry', 'queue:restart', 'vendor:publish', 'lang:publish', 'stub:publish', 'package:discover', 'schedule:clear-cache', 'test'];
    if (in_array($command, $read, true)) {
        return 'read';
    }
    return str_starts_with($command, 'make:') || in_array($command, $write, true) ? 'write' : 'full';
};
$longRunning = ['tinker' => 'Use the run_tinker tool instead of artisan tinker.', 'serve' => 'artisan serve keeps running; start it in a terminal.', 'queue:work' => 'Workers keep running; start them in a terminal.', 'queue:listen' => 'Workers keep running; start them in a terminal.', 'schedule:work' => 'schedule:work keeps running; start it in a terminal.', 'pail' => 'pail keeps running; use read_logs.', 'horizon' => 'Horizon keeps running; start it in a terminal.', 'reverb:start' => 'Reverb keeps running; start it in a terminal.', 'octane:start' => 'Octane keeps running; start it in a terminal.', 'dev' => 'dev keeps running; start it in a terminal.'];

$tool('access_level', 'read', 'The access level the user gave this MCP server for this project (read, write or full), which tools and Artisan commands each level unlocks, and where the user changes it.', [], function () use (&$tools, $level) {
    $by = ['read' => [], 'write' => [], 'full' => []];
    foreach ($tools as $name => $t) {
        $by[$t['level']][] = $name;
    }
    return [
        'level' => $level,
        'tools' => $by,
        'artisan' => [
            'read' => 'read-only commands: about, list, help, route:list, migrate:status, schedule:list, event:list, model:show, db:show, db:table, channel:list, queue:failed, queue:monitor',
            'write' => 'make:*, migrate, db:seed, the cache, config, route, view and event cache commands, optimize, storage:link, queue:retry, queue:restart, vendor:publish',
            'full' => 'every other command, such as migrate:fresh, migrate:rollback, db:wipe, key:generate, config:show or schedule:run',
        ],
        'change' => 'Only the user can change it, in OpenIDEA: Settings › Plugins › Laravel OpenIDEA › MCP access. New calls use it right away.',
    ];
});

$tool('list_classes', 'read', 'Classes of the app by kind, as the hub sections list them: controller, request, resource, middleware, model, enum, cast, rule, scope, observer, event, listener, job, mail, notification, policy, provider, command, component, livewire, factory, seeder, exception, interface, trait. Each with file:line, parent, interfaces, public methods and, where it applies, enum cases, the command signature or the events a listener handles. Without a kind it counts each kind.', ['kind' => $str('One of the kinds above'), 'filter' => $str('Text to match in the class name')], function (array $a) use ($boot, $appClasses, $like, $rel) {
    $boot();
    $is = static fn (ReflectionClass $r, string $parent): bool => (class_exists($parent) || interface_exists($parent)) && $r->getName() !== $parent && $r->isSubclassOf($parent);
    $in = static fn (ReflectionClass $r, string $segment): bool => str_contains('\\' . $r->getName(), '\\' . $segment . '\\');
    $kinds = [
        'controller' => static fn ($r) => $in($r, 'Http\\Controllers') && ! $r->isAbstract() && ! $r->isInterface() && ! $r->isTrait(),
        'request' => static fn ($r) => $is($r, 'Illuminate\\Foundation\\Http\\FormRequest'),
        'resource' => static fn ($r) => $is($r, 'Illuminate\\Http\\Resources\\Json\\JsonResource'),
        'middleware' => static fn ($r) => $in($r, 'Http\\Middleware') && ! $r->isInterface() && ! $r->isTrait(),
        'model' => static fn ($r) => $is($r, 'Illuminate\\Database\\Eloquent\\Model') && ! $r->isAbstract(),
        'enum' => static fn ($r) => $r->isEnum(),
        'cast' => static fn ($r) => $is($r, 'Illuminate\\Contracts\\Database\\Eloquent\\CastsAttributes') || $is($r, 'Illuminate\\Contracts\\Database\\Eloquent\\CastsInboundAttributes'),
        'rule' => static fn ($r) => $is($r, 'Illuminate\\Contracts\\Validation\\ValidationRule') || $is($r, 'Illuminate\\Contracts\\Validation\\Rule') || $is($r, 'Illuminate\\Contracts\\Validation\\InvokableRule'),
        'scope' => static fn ($r) => $is($r, 'Illuminate\\Database\\Eloquent\\Scope'),
        'observer' => static fn ($r) => $in($r, 'Observers'),
        'event' => static fn ($r) => $in($r, 'Events') && ! $r->isEnum(),
        'listener' => static fn ($r) => $in($r, 'Listeners'),
        'job' => static fn ($r) => $in($r, 'Jobs'),
        'mail' => static fn ($r) => $is($r, 'Illuminate\\Mail\\Mailable'),
        'notification' => static fn ($r) => $is($r, 'Illuminate\\Notifications\\Notification'),
        'policy' => static fn ($r) => $in($r, 'Policies'),
        'provider' => static fn ($r) => $is($r, 'Illuminate\\Support\\ServiceProvider'),
        'command' => static fn ($r) => $is($r, 'Illuminate\\Console\\Command') && ! $r->isAbstract(),
        'component' => static fn ($r) => $is($r, 'Illuminate\\View\\Component') && ! $r->isAbstract(),
        'livewire' => static fn ($r) => $is($r, 'Livewire\\Component') && ! $r->isAbstract(),
        'factory' => static fn ($r) => $is($r, 'Illuminate\\Database\\Eloquent\\Factories\\Factory') && ! $r->isAbstract(),
        'seeder' => static fn ($r) => $is($r, 'Illuminate\\Database\\Seeder'),
        'exception' => static fn ($r) => $r->implementsInterface(Throwable::class),
        'interface' => static fn ($r) => $r->isInterface(),
        'trait' => static fn ($r) => $r->isTrait(),
    ];
    $all = $appClasses(static fn () => true, ['app', 'database']);
    $kind = strtolower(trim((string) ($a['kind'] ?? '')));
    if ($kind === '') {
        $counts = [];
        foreach ($kinds as $name => $test) {
            $counts[$name] = count(array_filter($all, static fn ($r) => $test($r)));
        }
        return ['counts' => array_filter($counts), 'hint' => 'Call list_classes with one of these kinds.'];
    }
    if (! isset($kinds[$kind])) {
        throw new InvalidArgumentException('Unknown kind ' . $kind . '; use one of ' . implode(', ', array_keys($kinds)));
    }
    $out = [];
    foreach ($all as $r) {
        if (! $kinds[$kind]($r) || ! $like($a['filter'] ?? null, $r->getName())) {
            continue;
        }
        $parent = $r->getParentClass();
        $own = array_values(array_diff($r->getInterfaceNames(), $parent ? $parent->getInterfaceNames() : []));
        $methods = [];
        foreach ($r->getMethods(ReflectionMethod::IS_PUBLIC) as $m) {
            if ($m->class === $r->getName() && (! str_starts_with($m->name, '__') || $m->name === '__invoke')) {
                $methods[] = $m->name;
            }
        }
        $row = ['class' => $r->getName(), 'file' => $rel($r->getFileName()) . ':' . $r->getStartLine(), 'extends' => $parent ? $parent->getName() : null, 'implements' => $own, 'methods' => $methods];
        if ($r->isEnum()) {
            $row['cases'] = array_map(static fn ($c) => $c instanceof BackedEnum ? [$c->name => $c->value] : $c->name, ($r->getName())::cases());
        }
        if ($kind === 'command') {
            $defaults = $r->getDefaultProperties();
            $row['signature'] = trim((string) ($defaults['signature'] ?? $defaults['name'] ?? ''));
        }
        if (in_array($kind, ['listener', 'observer'], true) && $r->hasMethod('handle')) {
            $type = $r->getMethod('handle')->getParameters()[0] ?? null;
            $row['handles'] = $type && $type->getType() ? (string) $type->getType() : null;
        }
        $out[] = $row;
    }
    return $out;
});

$tool('check_env_keys', 'read', 'Keys missing between .env and .env.example, by name only; values are never read out.', [], function () use ($base) {
    $keys = static function (string $file): ?array {
        if (! is_file($file)) {
            return null;
        }
        preg_match_all('/^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_.]*)\s*=/m', (string) file_get_contents($file), $m);
        return array_values(array_unique($m[1]));
    };
    $env = $keys($base . '/.env');
    $example = $keys($base . '/.env.example');
    return [
        'env_exists' => $env !== null,
        'example_exists' => $example !== null,
        'missing_in_env' => array_values(array_diff($example ?? [], $env ?? [])),
        'missing_in_example' => array_values(array_diff($env ?? [], $example ?? [])),
    ];
});

$tool('list_tests', 'read', 'Pest and PHPUnit tests under tests/ with file, line, name and kind. Filter matches the name or the file.', ['filter' => $str('Text to match')], function (array $a) use ($base, $like) {
    $out = [];
    if (! is_dir($base . '/tests')) {
        return $out;
    }
    foreach (new RecursiveIteratorIterator(new RecursiveDirectoryIterator($base . '/tests', FilesystemIterator::SKIP_DOTS)) as $f) {
        if (! str_ends_with($f->getFilename(), '.php')) {
            continue;
        }
        $code = (string) file_get_contents($f->getPathname());
        $file = substr($f->getPathname(), strlen($base) + 1);
        $found = [];
        preg_match_all('/^\s*(it|test)\(\s*([\'"])(.*?)(?<!\\\\)\2/m', $code, $m, PREG_SET_ORDER | PREG_OFFSET_CAPTURE);
        foreach ($m as $hit) {
            $found[] = [$hit[0][1], ($hit[1][0] === 'it' ? 'it ' : '') . $hit[3][0], 'pest'];
        }
        preg_match_all('/(#\[Test\]\s*(?:#\[[^\]]*\]\s*)*)?public\s+function\s+(\w+)\s*\(/', $code, $m, PREG_SET_ORDER | PREG_OFFSET_CAPTURE);
        foreach ($m as $hit) {
            if (str_starts_with($hit[2][0], 'test') || ($hit[1][0] ?? '') !== '') {
                $found[] = [$hit[2][1], $hit[2][0], 'phpunit'];
            }
        }
        foreach ($found as [$offset, $name, $kind]) {
            if ($like($a['filter'] ?? null, $name, $file)) {
                $out[] = ['file' => $file, 'line' => substr_count($code, "\n", 0, $offset) + 1, 'name' => $name, 'kind' => $kind];
            }
        }
        if (count($out) >= 3000) {
            break;
        }
    }
    return $out;
});

$tool('run_phpstan', 'read', 'Runs the project\'s PHPStan (with Larastan when phpstan.neon includes it) on some paths, or on the paths of its config, and returns the errors per file and line. It analyses code without running it.', ['paths' => $list('Files or folders relative to the project root; empty uses phpstan.neon'), 'level' => $int('Rule level 0-10; default: the config')], function (array $a) use ($base, $run, $inside) {
    if (! is_file($base . '/vendor/bin/phpstan')) {
        throw new RuntimeException('PHPStan is not installed; the user can add it with composer require --dev larastan/larastan');
    }
    $paths = array_map($inside, (array) ($a['paths'] ?? []));
    $argv = [PHP_BINARY, 'vendor/bin/phpstan', 'analyse', '--error-format=json', '--no-progress', '--no-interaction', '--memory-limit=2G'];
    if (isset($a['level']) && $a['level'] !== '') {
        $argv[] = '--level=' . max(0, min(10, (int) $a['level']));
    }
    $result = $run(array_merge($argv, $paths), 300, null, 8_000_000);
    $start = strpos($result['output'], '{');
    $report = $start === false ? null : json_decode(substr($result['output'], $start), true);
    if (! is_array($report)) {
        return ['ran' => false] + $result;
    }
    $files = [];
    foreach ($report['files'] ?? [] as $file => $data) {
        $files[] = ['file' => ltrim(str_replace([$base, '/var/www/html'], '', $file), '/'), 'errors' => array_map(static fn ($m) => ['line' => $m['line'] ?? null, 'message' => $m['message'] ?? '', 'identifier' => $m['identifier'] ?? null, 'tip' => $m['tip'] ?? null], $data['messages'] ?? [])];
    }
    return ['totals' => $report['totals'] ?? null, 'files' => $files, 'general_errors' => $report['errors'] ?? []];
});

$tool('run_artisan', 'read', 'Runs an Artisan command and returns its output. Read-only commands (route:list, about, migrate:status, schedule:list, model:show, db:table…) work at every access level; generators (make:*), migrate, db:seed and the cache and optimize commands need write access; any other command (migrate:fresh, db:wipe, key:generate, config:show…) needs full access. Interactive and long-running commands (tinker, serve, queue:work…) are refused. Call access_level for the exact lists.', ['command' => $str('Command name, for example "make:model" or "route:list"'), 'arguments' => $list('Arguments and options, for example ["Post", "--migration"]')], function (array $a) use ($run, $artisanLevel, $longRunning, $allows, $denied) {
    $parts = preg_split('/\s+/', trim((string) ($a['command'] ?? ''))) ?: [];
    $command = (string) array_shift($parts);
    if ($command === '' || str_starts_with($command, '-')) {
        throw new InvalidArgumentException('Give a command name such as route:list');
    }
    if (isset($longRunning[$command])) {
        throw new RuntimeException($longRunning[$command]);
    }
    $needed = $artisanLevel($command);
    if (! $allows($needed)) {
        throw $denied($needed, "artisan $command");
    }
    $args = array_merge($parts, array_map('strval', (array) ($a['arguments'] ?? [])));
    return ['command' => trim("artisan $command " . implode(' ', $args))] + $run(array_merge([PHP_BINARY, 'artisan', $command], $args, ['--no-interaction', '--no-ansi']), 300);
});

$tool('format_code', 'write', 'Formats PHP files or folders with the project\'s Pint (or PHP CS Fixer) and returns what changed; dry_run only reports what would change.', ['paths' => $list('Files or folders relative to the project root'), 'dry_run' => $bool('Only report the changes'), 'tool' => $str('pint or php-cs-fixer; default: whichever is installed')], function (array $a) use ($base, $run, $inside) {
    $paths = array_map($inside, (array) ($a['paths'] ?? []));
    if ($paths === []) {
        throw new InvalidArgumentException('Give the files or folders to format');
    }
    $tool = (string) ($a['tool'] ?? '');
    if ($tool === '') {
        $tool = is_file($base . '/vendor/bin/pint') ? 'pint' : (is_file($base . '/vendor/bin/php-cs-fixer') ? 'php-cs-fixer' : '');
    }
    $dry = (bool) ($a['dry_run'] ?? false);
    $argv = match ($tool) {
        'pint' => array_merge([PHP_BINARY, 'vendor/bin/pint'], $paths, $dry ? ['--test'] : [], ['-v', '--no-interaction']),
        'php-cs-fixer' => array_merge([PHP_BINARY, 'vendor/bin/php-cs-fixer', 'fix'], $paths, $dry ? ['--dry-run', '--diff'] : [], ['--no-interaction']),
        default => throw new RuntimeException('Neither Pint nor PHP CS Fixer is installed'),
    };
    if (! is_file($base . '/' . $argv[1])) {
        throw new RuntimeException("$tool is not installed");
    }
    return ['tool' => $tool, 'dry_run' => $dry] + $run($argv, 300);
});

$tool('run_tests', 'write', 'Runs the tests, or the ones matching a filter or path, with Pest or PHPUnit and returns the output. Tests can write to the testing database.', ['filter' => $str('Test name filter'), 'path' => $str('A test file or folder relative to the project root')], function (array $a) use ($base, $run, $inside) {
    $bin = is_file($base . '/vendor/bin/pest') ? 'vendor/bin/pest' : (is_file($base . '/vendor/bin/phpunit') ? 'vendor/bin/phpunit' : null);
    if ($bin === null) {
        throw new RuntimeException('Neither Pest nor PHPUnit is installed');
    }
    $argv = [PHP_BINARY, $bin, '--colors=never'];
    if (! empty($a['path'])) {
        $argv[] = $inside($a['path']);
    }
    if (! empty($a['filter'])) {
        array_push($argv, '--filter', (string) $a['filter']);
    }
    return $run($argv, 600);
});

$tool('query_to_sql', 'write', 'Shows the SQL an Eloquent or query builder expression runs, with the bindings filled in, without running it: queries are captured with DB::pretend() and model events are off. Takes one expression on models or DB with literal values, for example Post::where(\'published\', true)->with(\'author\')->latest()->get().', ['code' => $str('One Eloquent or DB query builder expression')], function (array $a) use ($boot) {
    $code = rtrim(trim((string) ($a['code'] ?? '')), "; \n\t");
    if ($code === '') {
        throw new InvalidArgumentException('Give a query expression');
    }
    $tokens = array_slice(token_get_all('<?php ' . $code . ';'), 1);
    $tokens = array_values(array_filter($tokens, static fn ($t) => ! is_array($t) || ! in_array($t[0], [T_WHITESPACE, T_COMMENT, T_DOC_COMMENT], true)));
    array_pop($tokens);
    $allowed = [T_STRING, T_NAME_QUALIFIED, T_NAME_FULLY_QUALIFIED, T_DOUBLE_COLON, T_OBJECT_OPERATOR, T_NULLSAFE_OBJECT_OPERATOR, T_CONSTANT_ENCAPSED_STRING, T_LNUMBER, T_DNUMBER, T_VARIABLE, T_FN, T_DOUBLE_ARROW, T_CLASS, T_IS_EQUAL, T_IS_NOT_EQUAL, T_IS_IDENTICAL, T_IS_NOT_IDENTICAL, T_IS_SMALLER_OR_EQUAL, T_IS_GREATER_OR_EQUAL, T_BOOLEAN_AND, T_BOOLEAN_OR, T_COALESCE];
    $chars = ['(', ')', '[', ']', ',', '.', '-', '+', '*', '/', '!', '<', '>', '?', ':'];
    $params = [];
    foreach ($tokens as $i => $t) {
        if (is_array($t) && $t[0] === T_FN) {
            for ($j = $i + 2, $depth = 1; isset($tokens[$j]) && $depth > 0; $j++) {
                $depth += $tokens[$j] === '(' ? 1 : ($tokens[$j] === ')' ? -1 : 0);
                if (is_array($tokens[$j]) && $tokens[$j][0] === T_VARIABLE) {
                    $params[$tokens[$j][1]] = true;
                }
            }
        }
    }
    $statics = [];
    $refuse = static fn (string $what) => new InvalidArgumentException("query_to_sql takes one query expression on models or DB with literal values; $what isn't allowed");
    $banned = ['getpdo', 'getreadpdo', 'getrawpdo', 'getrawreadpdo', 'setpdo', 'setreadpdo', 'getconnection', 'resolveconnection', 'getconnectionresolver', 'setconnectionresolver', 'geteventdispatcher', 'seteventdispatcher', 'getfacaderoot', 'getfacadeapplication', 'setfacadeapplication', 'swap', 'macro', 'mixin', 'extend', 'getschemabuilder', 'getapplication', 'setapplication', 'make', 'app', 'call', 'dispatch', '__call', '__callstatic', '__invoke'];
    $pure = ['date', 'time', 'count', 'key', 'max', 'min', 'sum', 'current', 'end', 'next', 'reset', 'sort', 'round', 'floor', 'ceil', 'abs', 'trim', 'strlen', 'strtolower', 'strtoupper', 'ucfirst', 'lcfirst', 'intval', 'floatval', 'strval', 'boolval', 'json_encode', 'md5', 'sha1'];
    foreach ($tokens as $i => $t) {
        $text = is_array($t) ? $t[1] : $t;
        if (! is_array($t)) {
            if (! in_array($t, $chars, true)) {
                throw $refuse("\"$text\"");
            }
            continue;
        }
        if (! in_array($t[0], $allowed, true)) {
            throw $refuse("\"$text\"");
        }
        $prev = $tokens[$i - 1] ?? null;
        $next = $tokens[$i + 1] ?? null;
        $member = is_array($prev) && in_array($prev[0], [T_OBJECT_OPERATOR, T_NULLSAFE_OBJECT_OPERATOR, T_DOUBLE_COLON], true);
        if ($t[0] === T_VARIABLE && (! isset($params[$text]) || $next === '(')) {
            throw $refuse($next === '(' ? "calling $text()" : "the variable $text (use a literal value)");
        }
        if ($t[0] === T_CONSTANT_ENCAPSED_STRING) {
            $value = substr($text, 1, -1);
            if (str_contains($value, '::') || str_contains($value, '@') || in_array(strtolower($value), $banned, true) || (is_callable($value) && ! in_array(strtolower($value), $pure, true))) {
                throw $refuse("the string $text, which names something callable");
            }
        }
        if ($t[0] === T_STRING && $member && in_array(strtolower($text), $banned, true)) {
            throw $refuse("->$text()");
        }
        if (in_array($t[0], [T_STRING, T_NAME_QUALIFIED, T_NAME_FULLY_QUALIFIED], true) && ! $member) {
            if (is_array($next) && $next[0] === T_DOUBLE_COLON) {
                $statics[] = $text;
            } elseif ($next === '(' && ! in_array(strtolower($text), ['now', 'today'], true)) {
                throw $refuse("the function call $text()");
            }
        }
    }
    $boot();
    $connections = [config('database.default')];
    foreach (array_unique($statics) as $name) {
        $short = ltrim($name, '\\');
        if (in_array($short, ['DB', 'Illuminate\\Support\\Facades\\DB'], true)) {
            if (! class_exists('DB')) {
                class_alias(Illuminate\Support\Facades\DB::class, 'DB');
            }
            continue;
        }
        $found = null;
        foreach ([$short, 'App\\Models\\' . $short, 'App\\' . $short] as $candidate) {
            if (class_exists($candidate) && is_subclass_of($candidate, Illuminate\Database\Eloquent\Model::class)) {
                $found = $candidate;
                break;
            }
        }
        if ($found === null) {
            throw $refuse("$name (not an Eloquent model or DB)");
        }
        if ($found !== $short && ! class_exists($short, false) && ! str_contains($short, '\\')) {
            class_alias($found, $short);
        }
        $connections[] = (new $found())->getConnectionName() ?? config('database.default');
    }
    $connections = array_values(array_unique($connections));
    $value = null;
    $queries = [];
    $capture = static function (array $names) use (&$capture, &$value, &$queries, $code): void {
        if ($names === []) {
            $value = Illuminate\Database\Eloquent\Model::withoutEvents(static fn () => eval('return ' . $code . ';'));
            return;
        }
        $name = array_shift($names);
        $logged = Illuminate\Support\Facades\DB::connection($name)->pretend(static function () use ($capture, $names) {
            $capture($names);
        });
        foreach ($logged as $q) {
            $queries[] = $q + ['connection' => $name];
        }
    };
    $capture($connections);
    $raw = static function (string $sql, array $bindings): string {
        foreach ($bindings as $b) {
            $b = match (true) {
                $b === null => 'NULL',
                is_bool($b) => $b ? '1' : '0',
                is_int($b), is_float($b) => (string) $b,
                $b instanceof DateTimeInterface => "'" . $b->format('Y-m-d H:i:s') . "'",
                $b instanceof BackedEnum => is_string($b->value) ? "'" . str_replace("'", "''", $b->value) . "'" : (string) $b->value,
                default => "'" . str_replace("'", "''", (string) $b) . "'",
            };
            $at = strpos($sql, '?');
            if ($at === false) {
                break;
            }
            $sql = substr_replace($sql, $b, $at, 1);
        }
        return $sql;
    };
    $sql = array_map(static fn ($q) => $raw($q['query'], $q['bindings'] ?? []), $queries);
    if ($sql === [] && ($value instanceof Illuminate\Database\Eloquent\Builder || $value instanceof Illuminate\Database\Query\Builder || $value instanceof Illuminate\Database\Eloquent\Relations\Relation)) {
        $builder = $value instanceof Illuminate\Database\Query\Builder ? $value : $value->toBase();
        $sql[] = $raw($builder->toSql(), $builder->getBindings());
    }
    return ['sql' => $sql, 'connections' => $connections, 'note' => 'Nothing ran: the queries were captured with DB::pretend(), so reads returned no rows and writes changed nothing.'];
});

$tool('audit_recorder', 'write', 'Turns the Live Audit recorder on ("on") or off ("off"), or reports it ("status"). While on, requests, SQL, events, jobs, cache, mail, HTTP client calls, logs and exceptions of the local app are recorded for audit_recent.', ['state' => $str('on, off or status')], function (array $a) use ($base, $run) {
    $script = $base . '/storage/openidea/install.php';
    $source = $base . '/storage/openidea/recorder.source';
    $action = ['on' => 'install', 'off' => 'uninstall', 'status' => 'status'][(string) ($a['state'] ?? 'status')] ?? null;
    if ($action === null) {
        throw new InvalidArgumentException('state is on, off or status');
    }
    if (! is_file($script) || ($action === 'install' && ! is_file($source))) {
        throw new RuntimeException('The recorder files are not here yet: the user can open the project in OpenIDEA with the latest Laravel OpenIDEA, or turn the recorder on in the hub (Live Audit).');
    }
    $result = $run([PHP_BINARY, $script, $action], 60, $action === 'install' ? (string) file_get_contents($source) : null);
    $at = strrpos($result['output'], '@@openidea@@');
    $status = $at === false ? null : json_decode(trim(substr($result['output'], $at + 12)), true);
    return is_array($status) ? $status : $result;
});

$sweep = static function (float $since) use ($base): void {
    $uid = @fileowner($base);
    $gid = @filegroup($base);
    if ($uid === false || $uid === 0 || ! function_exists('posix_geteuid') || posix_geteuid() !== 0) {
        return;
    }
    foreach (['app', 'bootstrap/cache', 'config', 'database', 'lang', 'resources', 'routes', 'tests', 'public', 'storage/framework', 'storage/logs'] as $dir) {
        if (! is_dir($base . '/' . $dir)) {
            continue;
        }
        try {
            $items = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($base . '/' . $dir, FilesystemIterator::SKIP_DOTS), RecursiveIteratorIterator::SELF_FIRST);
            foreach ($items as $item) {
                if ($item->getOwner() === 0 && $item->getCTime() >= (int) $since - 1) {
                    @chown($item->getPathname(), $uid);
                    @chgrp($item->getPathname(), $gid);
                }
            }
        } catch (Throwable) {
        }
    }
};

$msg = json_decode((string) stream_get_contents(STDIN), true);
$msg = is_array($msg) ? $msg : [];
$id = $msg['id'] ?? null;
$method = (string) ($msg['method'] ?? '');
try {
    switch ($method) {
        case 'tools/list':
            $out = [];
            foreach ($tools as $name => $t) {
                if ($allows($t['level'])) {
                    $out[] = ['name' => $name, 'description' => $t['description'], 'inputSchema' => ['type' => 'object', 'properties' => $t['properties'] ?: new stdClass()]];
                }
            }
            $send(['jsonrpc' => '2.0', 'id' => $id, 'result' => ['tools' => $out]]);
            break;
        case 'tools/call':
            $name = (string) ($msg['params']['name'] ?? '');
            if (! isset($tools[$name])) {
                $send(['jsonrpc' => '2.0', 'id' => $id, 'error' => ['code' => -32602, 'message' => "Unknown tool $name"]]);
                break;
            }
            $started = microtime(true);
            ob_start();
            try {
                if (! $allows($tools[$name]['level'])) {
                    throw $denied($tools[$name]['level'], $name);
                }
                $result = ($tools[$name]['run'])((array) ($msg['params']['arguments'] ?? []));
                $text = json_encode($result, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR);
                $isError = false;
            } catch (Throwable $e) {
                $text = get_class($e) . ': ' . $e->getMessage();
                $isError = true;
            } finally {
                while (ob_get_level() > 0) {
                    ob_end_clean();
                }
            }
            if ($tools[$name]['level'] !== 'read' || $name === 'run_artisan') {
                $sweep($started);
            }
            if (strlen((string) $text) > 400_000) {
                $text = substr((string) $text, 0, 400_000) . "\n… truncated; use a filter";
            }
            $send(['jsonrpc' => '2.0', 'id' => $id, 'result' => ['content' => [['type' => 'text', 'text' => (string) $text]], 'isError' => $isError]]);
            break;
        default:
            $send(['jsonrpc' => '2.0', 'id' => $id, 'error' => ['code' => -32601, 'message' => "Method not found: $method"]]);
    }
} catch (Throwable $e) {
    while (ob_get_level() > 0) {
        ob_end_clean();
    }
    $send(['jsonrpc' => '2.0', 'id' => $id, 'error' => ['code' => -32603, 'message' => $e->getMessage()]]);
}
