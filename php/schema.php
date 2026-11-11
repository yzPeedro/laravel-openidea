<?php

$__openideaInput = json_decode((string) stream_get_contents(STDIN), true) ?: [];
$__openideaBase = getcwd();
error_reporting(E_ALL & ~E_DEPRECATED & ~E_USER_DEPRECATED);

$__openideaReply = static function (array $payload): void {
    fwrite(STDOUT, "\n@@openidea-schema@@" . json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR) . "\n");
};

try {
    require $__openideaBase . '/vendor/autoload.php';
    $app = require $__openideaBase . '/bootstrap/app.php';
    $app->make(Illuminate\Contracts\Console\Kernel::class)->bootstrap();
} catch (Throwable $e) {
    $__openideaReply(['error' => 'The app did not boot: ' . $e->getMessage()]);
    exit(0);
}

$action = (string) ($__openideaInput['action'] ?? 'live');
$connectionName = (string) ($__openideaInput['connection'] ?? '');
$connection = app('db')->connection($connectionName === '' ? null : $connectionName);
$driver = $connection->getDriverName();
$builder = $connection->getSchemaBuilder();

$secret = '/^(pass|passwd|passwords?|passphrase|passcode|pwd|secrets?|tokens?|apikey|authorization|cookies?|credentials?|private|signature|otp|cvv|cvc|card|cardholder|creditcard|hash|salt)$/';
$pii = '/^(emails?|cpf|cnpj|rg|documents?|phones?|telefones?|celular|mobile|birth|birthday|birthdate|nascimento|address(es)?|endereco|cep|zip|zipcode|ssn|passport|ip)$/';
$showPiiFlag = env('OPENIDEA_AUDIT_SHOW_PII');
$showPii = $showPiiFlag === true || $showPiiFlag === 'true' || $showPiiFlag === '1';

$masked = static function (string $column) use ($secret, $pii, $showPii): bool {
    $spaced = preg_replace(['/([a-z0-9])([A-Z])/', '/([a-zA-Z])([0-9])/'], '$1 $2', $column);
    $words = preg_split('/[^a-z0-9]+/', strtolower($spaced ?? $column), -1, PREG_SPLIT_NO_EMPTY) ?: [];
    if ($words === []) {
        return false;
    }
    if (end($words) === 'key') {
        return true;
    }
    $candidates = $words;
    for ($i = 1; $i < count($words); $i++) {
        $candidates[] = $words[$i - 1] . $words[$i];
    }
    foreach ($candidates as $word) {
        if (preg_match($secret, $word) || (! $showPii && preg_match($pii, $word))) {
            return true;
        }
    }
    return false;
};

$tableNames = static function () use ($builder, $connection, $driver): array {
    if (method_exists($builder, 'getTables')) {
        $names = [];
        foreach ($builder->getTables() as $t) {
            $t = (array) $t;
            $names[] = (string) $t['name'];
        }
        return $names;
    }
    $rows = match ($driver) {
        'mysql', 'mariadb' => $connection->select('SHOW TABLES'),
        'sqlite' => $connection->select("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'"),
        'pgsql' => $connection->select('SELECT tablename AS name FROM pg_tables WHERE schemaname = current_schema()'),
        'sqlsrv' => $connection->select('SELECT name FROM sys.tables'),
        default => [],
    };
    return array_map(static fn ($r) => (string) array_values((array) $r)[0], $rows);
};

$describe = static function (string $table) use ($builder): array {
    $columns = [];
    if (method_exists($builder, 'getColumns')) {
        foreach ($builder->getColumns($table) as $c) {
            $c = (array) $c;
            $columns[] = ['name' => (string) $c['name'], 'type' => (string) ($c['type_name'] ?? $c['type'] ?? ''), 'nullable' => (bool) ($c['nullable'] ?? false)];
        }
    } else {
        foreach ($builder->getColumnListing($table) as $name) {
            $columns[] = ['name' => (string) $name, 'type' => '', 'nullable' => false];
        }
    }
    $indexes = [];
    if (method_exists($builder, 'getIndexes')) {
        foreach ($builder->getIndexes($table) as $i) {
            $i = (array) $i;
            $indexes[] = ['name' => (string) $i['name'], 'columns' => array_values((array) $i['columns']), 'unique' => (bool) ($i['unique'] ?? false), 'primary' => (bool) ($i['primary'] ?? false)];
        }
    }
    return ['columns' => $columns, 'indexes' => $indexes];
};

try {
    if ($action === 'live') {
        $tables = [];
        foreach ($tableNames() as $name) {
            $tables[] = ['name' => $name] + $describe($name);
        }
        $pending = [];
        $migrator = app('migrator');
        $paths = array_merge($migrator->paths(), [database_path('migrations')]);
        $files = $migrator->getMigrationFiles($paths);
        $ran = $migrator->repositoryExists() ? $migrator->getRepository()->getRan() : [];
        foreach (array_keys($files) as $migration) {
            if (! in_array($migration, $ran, true)) {
                $pending[] = $migration;
            }
        }
        $__openideaReply([
            'connection' => $connection->getName(),
            'driver' => $driver,
            'database' => $connection->getDatabaseName(),
            'tables' => $tables,
            'pending' => $pending,
            'repository' => $migrator->repositoryExists(),
        ]);
    } elseif ($action === 'rows') {
        $table = (string) ($__openideaInput['table'] ?? '');
        if (! in_array($table, $tableNames(), true)) {
            $__openideaReply(['error' => "The table $table does not exist in the {$connection->getName()} connection."]);
            exit(0);
        }
        $size = max(10, min(200, (int) ($__openideaInput['size'] ?? 50)));
        $page = max(0, (int) ($__openideaInput['page'] ?? 0));
        $columns = array_column($describe($table)['columns'], 'name');
        $query = $connection->table($table);
        $total = (clone $query)->count();
        $rows = [];
        foreach ($query->offset($page * $size)->limit($size)->get() as $row) {
            $cells = [];
            foreach ((array) $row as $column => $value) {
                if ($value !== null && $masked((string) $column)) {
                    $cells[$column] = '•••';
                } elseif (is_string($value) && ! mb_check_encoding($value, 'UTF-8')) {
                    $cells[$column] = '(binary, ' . strlen($value) . ' bytes)';
                } elseif (is_string($value) && mb_strlen($value) > 300) {
                    $cells[$column] = mb_substr($value, 0, 300) . '…';
                } else {
                    $cells[$column] = $value;
                }
            }
            $rows[] = $cells;
        }
        $__openideaReply(['table' => $table, 'columns' => $columns ?: array_keys($rows[0] ?? []), 'rows' => $rows, 'total' => $total, 'page' => $page, 'size' => $size, 'masked' => array_values(array_filter($columns, $masked))]);
    } elseif ($action === 'explain') {
        $sql = trim((string) ($__openideaInput['sql'] ?? ''));
        $bindings = array_values((array) ($__openideaInput['bindings'] ?? []));
        if (! preg_match('/^select\b/i', $sql) || str_contains(rtrim($sql, "; \n\t"), ';')) {
            $__openideaReply(['error' => 'Only a single SELECT statement can be explained.']);
            exit(0);
        }
        $prefix = match ($driver) {
            'sqlite' => 'EXPLAIN QUERY PLAN ',
            'mysql', 'mariadb', 'pgsql' => 'EXPLAIN ',
            default => null,
        };
        if ($prefix === null) {
            $__openideaReply(['error' => "EXPLAIN is not supported for the $driver driver."]);
            exit(0);
        }
        $connection->beginTransaction();
        try {
            $plan = array_map(static fn ($r) => (array) $r, $connection->select($prefix . $sql, $bindings));
        } finally {
            $connection->rollBack();
        }
        $__openideaReply(['driver' => $driver, 'plan' => $plan]);
    } else {
        $__openideaReply(['error' => "Unknown schema action $action."]);
    }
} catch (Throwable $e) {
    $__openideaReply(['error' => $e->getMessage()]);
}
