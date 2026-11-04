<?php

$base = getcwd();
if (! is_file($base . '/artisan')) {
    $base = dirname(__DIR__, 2);
}
chdir($base);
error_reporting(E_ALL & ~E_DEPRECATED & ~E_USER_DEPRECATED);
ini_set('display_errors', 'stderr');

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

$appClasses = static function (callable $keep) use ($base): array {
    $map = is_file($base . '/vendor/composer/autoload_classmap.php') ? require $base . '/vendor/composer/autoload_classmap.php' : [];
    $out = [];
    foreach ($map as $class => $file) {
        if (! str_starts_with((string) realpath($file), $base . '/app/') && ! str_starts_with($file, $base . '/app/')) {
            continue;
        }
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
    if ($out === []) {
        foreach (new RecursiveIteratorIterator(new RecursiveDirectoryIterator($base . '/app', FilesystemIterator::SKIP_DOTS)) as $f) {
            if ($f->getExtension() !== 'php') {
                continue;
            }
            $code = (string) file_get_contents($f->getPathname());
            if (! preg_match('/^namespace\s+([^;]+);/m', $code, $ns) || ! preg_match('/^\s*(?:final\s+|abstract\s+|readonly\s+)*(?:class|interface|trait|enum)\s+(\w+)/m', $code, $cl)) {
                continue;
            }
            $class = $ns[1] . '\\' . $cl[1];
            try {
                if (class_exists($class) || interface_exists($class) || trait_exists($class) || enum_exists($class)) {
                    $r = new ReflectionClass($class);
                    if ($keep($r)) {
                        $out[] = $r;
                    }
                }
            } catch (Throwable) {
            }
        }
    }
    return $out;
};

$rel = static fn (?string $file): ?string => $file ? ltrim(str_replace([$base, '/var/www/html'], '', $file), '/') : null;

$tools = [];
$tool = static function (string $name, string $description, array $properties, callable $run) use (&$tools) {
    $tools[$name] = ['description' => $description, 'properties' => $properties, 'run' => $run];
};
$str = static fn (string $d) => ['type' => 'string', 'description' => $d];
$int = static fn (string $d) => ['type' => 'integer', 'description' => $d];

$tool('app_overview', 'Laravel OpenIDEA hub overview: Laravel and PHP versions, environment, drivers (database, cache, queue, session, mail, broadcasting), first-party and popular packages installed, and counts of routes, models, events, listeners, commands and scheduled tasks. Start here.', [], function () use ($boot, $base) {
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

$tool('list_routes', 'Routes with method, URI, name, action (Controller@method), middleware and the controller file. Filter matches URI, name or action.', ['filter' => $str('Text to match'), 'method' => $str('GET, POST, PUT, PATCH or DELETE')], function (array $a) use ($boot, $like, $rel) {
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

$tool('list_events_and_listeners', 'Every event the application listens to and its listeners, as registered at runtime (event discovery, Event::listen, subscribers, model observers as eloquent.* events, queued listeners).', ['filter' => $str('Text to match in the event or listener')], function (array $a) use ($boot, $like, $label) {
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

$tool('list_models', 'Eloquent models with table, connection, primary key, fillable, guarded, hidden, casts, relations (method → type and related model), local scopes, global scopes and observers.', ['filter' => $str('Model name to match')], function (array $a) use ($boot, $appClasses, $like, $rel, $label) {
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

$tool('list_container_bindings', 'Service container bindings: abstract, whether shared (singleton), and the concrete class or closure location. Filter matches the abstract.', ['filter' => $str('Text to match')], function (array $a) use ($boot, $like, $label) {
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

$tool('list_schedule', 'Scheduled tasks with cron expression, command or job, description, next run, timezone, overlapping and one-server flags.', [], function () use ($boot) {
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

$tool('list_artisan_commands', 'Artisan commands with description and usage. Filter matches the name or description.', ['filter' => $str('Text to match')], function (array $a) use ($boot, $like) {
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

$tool('get_config', 'Configuration values for a key or a whole file (for example "database.connections.mysql" or "queue"). Secrets are masked.', ['key' => $str('Config key or file name')], function (array $a) use ($boot, $mask) {
    $boot();
    $key = (string) ($a['key'] ?? 'app');
    return ['key' => $key, 'value' => $mask(config($key), str_contains($key, '.') ? substr($key, strrpos($key, '.') + 1) : null)];
});

$tool('database_schema', 'Database tables, or the columns, indexes and foreign keys of one table, from the live connection.', ['table' => $str('Table name; leave empty to list tables'), 'connection' => $str('Connection name')], function (array $a) use ($boot) {
    $boot();
    $schema = Illuminate\Support\Facades\Schema::connection($a['connection'] ?? null);
    if (empty($a['table'])) {
        return array_map(static fn ($t) => ['name' => $t['name'], 'rows' => $t['rows'] ?? null, 'size' => $t['size'] ?? null], $schema->getTables());
    }
    return ['table' => $a['table'], 'columns' => $schema->getColumns($a['table']), 'indexes' => $schema->getIndexes($a['table']), 'foreign_keys' => $schema->getForeignKeys($a['table'])];
});

$tool('audit_recent', 'Recent records from the Live Audit recorder of the hub (requests with input and timing, SQL with bindings and the calling line, events with payloads and listeners, jobs, cache, mail, notifications, HTTP client, gate checks, logs, exceptions, dumps), plus insights: N+1 query patterns, slow queries, exceptions and failed jobs. Turn the recorder on in the hub first.', ['types' => $str('Comma separated types, for example "request,query,exception"'), 'limit' => $int('Records to return, newest last (default 200)'), 'filter' => $str('Text to match')], function (array $a) use ($base) {
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

$tool('read_logs', 'The last entries of the Laravel log (storage/logs/laravel.log or the newest daily file), optionally only one level.', ['lines' => $int('Entries to return (default 30)'), 'level' => $str('error, warning, info, debug…')], function (array $a) use ($base) {
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

$tool('list_queues', 'Queue connections, the default queue, queued job classes of the app with tries, timeout and queue, and the failed jobs count when the failer is configured.', [], function () use ($boot, $appClasses, $rel) {
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

$tool('missing_translations', 'Translation keys present in one locale but missing in another (PHP files in lang/ and JSON files).', ['locale' => $str('Only report keys missing in this locale')], function (array $a) use ($base) {
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

$tool('run_tinker', 'Runs PHP inside the booted application (like php artisan tinker) and returns each statement result, echo output and SQL queries. Use for quick checks in the local environment; avoid writes unless asked.', ['code' => $str('PHP statements')], function (array $a) use ($boot) {
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

$tool('hub_sections', 'What the Laravel OpenIDEA hub offers and where, so you can point the user to it.', [], fn () => [
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

$send = static function (array $message): void {
    fwrite(STDOUT, json_encode($message, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR) . "\n");
    fflush(STDOUT);
};

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
    $method = $msg['method'] ?? '';
    if ($id === null) {
        continue;
    }
    try {
        switch ($method) {
            case 'initialize':
                $send(['jsonrpc' => '2.0', 'id' => $id, 'result' => [
                    'protocolVersion' => $msg['params']['protocolVersion'] ?? '2025-06-18',
                    'capabilities' => ['tools' => ['listChanged' => false]],
                    'serverInfo' => ['name' => 'laravel-openidea', 'version' => '1.0.0'],
                    'instructions' => 'Runtime view of this Laravel application from the Laravel OpenIDEA hub: routes, events and listeners, models with relations, scopes and observers, container bindings, schedule, queues, config, schema, translations, logs, the live audit recorder and Tinker. Call app_overview first.',
                ]]);
                break;
            case 'ping':
                $send(['jsonrpc' => '2.0', 'id' => $id, 'result' => new stdClass()]);
                break;
            case 'tools/list':
                $list = [];
                foreach ($tools as $name => $t) {
                    $list[] = ['name' => $name, 'description' => $t['description'], 'inputSchema' => ['type' => 'object', 'properties' => $t['properties'] ?: new stdClass()]];
                }
                $send(['jsonrpc' => '2.0', 'id' => $id, 'result' => ['tools' => $list]]);
                break;
            case 'tools/call':
                $name = $msg['params']['name'] ?? '';
                if (! isset($tools[$name])) {
                    $send(['jsonrpc' => '2.0', 'id' => $id, 'error' => ['code' => -32602, 'message' => "Unknown tool $name"]]);
                    break;
                }
                ob_start();
                try {
                    $result = ($tools[$name]['run'])((array) ($msg['params']['arguments'] ?? []));
                    $text = json_encode($result, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR);
                    $isError = false;
                } catch (Throwable $e) {
                    $text = get_class($e) . ': ' . $e->getMessage();
                    $isError = true;
                } finally {
                    ob_end_clean();
                }
                if (strlen($text) > 400_000) {
                    $text = substr($text, 0, 400_000) . "\n… truncated; use a filter";
                }
                $send(['jsonrpc' => '2.0', 'id' => $id, 'result' => ['content' => [['type' => 'text', 'text' => $text]], 'isError' => $isError]]);
                break;
            default:
                $send(['jsonrpc' => '2.0', 'id' => $id, 'error' => ['code' => -32601, 'message' => "Method not found: $method"]]);
        }
    } catch (Throwable $e) {
        $send(['jsonrpc' => '2.0', 'id' => $id, 'error' => ['code' => -32603, 'message' => $e->getMessage()]]);
    }
}
