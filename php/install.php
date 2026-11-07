<?php

$base = getcwd();
$action = $argv[1] ?? 'status';
$dir = $base . '/storage/openidea';
$file = $dir . '/Recorder.php';
$autoload = $base . '/vendor/autoload.php';
$marker = '/* openidea-recorder */';

$emit = static function (array $data): void {
    echo "\n@@openidea@@" . json_encode($data, JSON_UNESCAPED_SLASHES) . "\n";
};

$unhook = static function (string $code) use ($marker): string {
    $quoted = preg_quote($marker, '/');
    return preg_replace('/\$__openideaLoader = (.*?);\s*' . $quoted . '.*?' . $quoted . '\s*return \$__openideaLoader;/s', 'return $1;', $code);
};

$own = static function (string $path) use ($base): void {
    $uid = @fileowner($base);
    $gid = @filegroup($base);
    if ($uid !== false && function_exists('posix_geteuid') && posix_geteuid() === 0) {
        @chown($path, $uid);
        @chgrp($path, $gid);
    }
};

$hooked = static fn (): bool => is_file($autoload) && str_contains((string) file_get_contents($autoload), $marker);

if ($action === 'install') {
    if (! is_file($autoload)) {
        $emit(['ok' => false, 'error' => 'vendor/autoload.php is missing; run composer install']);
        exit(1);
    }
    if (! is_dir($dir) && ! @mkdir($dir, 0775, true) && ! is_dir($dir)) {
        $emit(['ok' => false, 'error' => "cannot create $dir"]);
        exit(1);
    }
    file_put_contents($dir . '/.gitignore', "*\n");
    $own($dir);
    $own($dir . '/.gitignore');
    $source = stream_get_contents(STDIN);
    if (! str_contains($source, 'RecorderServiceProvider')) {
        $emit(['ok' => false, 'error' => 'empty recorder source']);
        exit(1);
    }
    file_put_contents($file, $source);
    $own($file);
    $lint = @shell_exec(escapeshellarg(PHP_BINARY) . ' -l ' . escapeshellarg($file) . ' 2>&1');
    if (! is_string($lint) || ! str_contains($lint, 'No syntax errors')) {
        @unlink($file);
        $emit(['ok' => false, 'error' => 'recorder does not compile on this PHP: ' . trim((string) $lint)]);
        exit(1);
    }
    $code = $unhook(file_get_contents($autoload));
    $hook = '$__openideaLoader = $1; ' . $marker . " if (is_file(\$__openideaRecorder = __DIR__ . '/../storage/openidea/Recorder.php')) { require_once \$__openideaRecorder; } " . $marker . ' return $__openideaLoader;';
    $patched = preg_replace('/return\s+(ComposerAutoloaderInit\w+::getLoader\(\));/', $hook, $code, 1, $count);
    if ($count !== 1) {
        $emit(['ok' => false, 'error' => 'vendor/autoload.php has an unexpected shape']);
        exit(1);
    }
    file_put_contents($autoload . '.openidea', $patched);
    $lint = @shell_exec(escapeshellarg(PHP_BINARY) . ' -l ' . escapeshellarg($autoload . '.openidea') . ' 2>&1');
    if (! is_string($lint) || ! str_contains($lint, 'No syntax errors')) {
        @unlink($autoload . '.openidea');
        $emit(['ok' => false, 'error' => 'could not hook vendor/autoload.php: ' . trim((string) $lint)]);
        exit(1);
    }
    rename($autoload . '.openidea', $autoload);
    $emit(['ok' => true, 'active' => true]);
    exit(0);
}

if ($action === 'uninstall') {
    if (is_file($autoload)) {
        file_put_contents($autoload, $unhook(file_get_contents($autoload)));
    }
    @unlink($file);
    if (! is_file($dir . '/mcp.php')) {
        @unlink($dir . '/.gitignore');
        @rmdir($dir);
    }
    $emit(['ok' => true, 'active' => false]);
    exit(0);
}


$agentMarker = ['<!-- laravel-openidea:start -->', '<!-- laravel-openidea:end -->'];
$jsonFiles = [
    '.mcp.json' => ['key' => 'mcpServers', 'stdio' => false, 'always' => true],
    '.cursor/mcp.json' => ['key' => 'mcpServers', 'stdio' => false, 'always' => false],
    '.vscode/mcp.json' => ['key' => 'servers', 'stdio' => true, 'always' => false],
    '.junie/mcp/mcp.json' => ['key' => 'mcpServers', 'stdio' => false, 'always' => false],
    '.gemini/settings.json' => ['key' => 'mcpServers', 'stdio' => false, 'always' => false],
];
$writeJson = static function (string $path, array $data) use ($own): void {
    $dir = dirname($path);
    if (! is_dir($dir)) {
        @mkdir($dir, 0775, true);
        $own($dir);
    }
    $json = (string) json_encode($data, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    $json = preg_replace_callback('/^(?: {4})+/m', static fn ($m) => str_repeat(' ', intdiv(strlen($m[0]), 2)), $json);
    file_put_contents($path, $json . "\n");
    $own($path);
};
$stripBlock = static function (string $text) use ($agentMarker): string {
    $a = strpos($text, $agentMarker[0]);
    $b = strpos($text, $agentMarker[1]);
    if ($a === false || $b === false || $b < $a) {
        return $text;
    }
    return rtrim(substr($text, 0, $a)) . "\n" . ltrim(substr($text, $b + strlen($agentMarker[1])));
};
$agentStatus = static function () use ($base, $jsonFiles, $agentMarker, $dir): array {
    $configured = [];
    foreach ($jsonFiles as $file => $spec) {
        $data = is_file($base . '/' . $file) ? json_decode((string) file_get_contents($base . '/' . $file), true) : null;
        if (isset($data[$spec['key']]['laravel-openidea'])) {
            $configured[] = $file;
        }
    }
    $guidelines = [];
    foreach (['AGENTS.md', 'CLAUDE.md', 'GEMINI.md', '.github/copilot-instructions.md', '.junie/guidelines.md', '.ai/guidelines/laravel-openidea.md'] as $f) {
        if (is_file($base . '/' . $f) && (str_contains((string) file_get_contents($base . '/' . $f), $agentMarker[0]) || str_ends_with($f, 'laravel-openidea.md'))) {
            $guidelines[] = $f;
        }
    }
    $boost = is_dir($base . '/vendor/laravel/boost');
    $boostFiles = array_values(array_filter(['boost.json', '.mcp.json', 'AGENTS.md', 'CLAUDE.md', '.cursor/mcp.json', '.vscode/mcp.json', '.junie/mcp/mcp.json', '.github/copilot-instructions.md', '.ai/guidelines'], static fn ($f) => file_exists($base . '/' . $f)));
    return ['server' => is_file($dir . '/mcp.php'), 'configured' => $configured, 'guidelines' => $guidelines, 'boost' => $boost, 'present' => $boostFiles];
};

$writeMcp = static function (array $payload) use ($dir, $own): void {
    $level = in_array($payload['access'] ?? 'read', ['read', 'write', 'full'], true) ? $payload['access'] : 'read';
    $files = [
        'mcp.php' => $payload['mcp'] ?? null,
        'install.php' => $payload['install'] ?? null,
        'recorder.source' => $payload['recorder'] ?? null,
        'mcp-access.json' => json_encode(['level' => $level]) . "\n",
    ];
    foreach ($files as $name => $content) {
        $path = $dir . '/' . $name;
        if (! is_string($content) || $content === '' || (is_file($path) && file_get_contents($path) === $content)) {
            continue;
        }
        file_put_contents($path, $content);
        $own($path);
    }
};

if ($action === 'mcp-sync') {
    $payload = json_decode((string) stream_get_contents(STDIN), true);
    if (! is_array($payload) || empty($payload['mcp']) || ! is_file($dir . '/mcp.php')) {
        $emit(['ok' => true, 'synced' => false]);
        exit(0);
    }
    $writeMcp($payload);
    $emit(['ok' => true, 'synced' => true]);
    exit(0);
}

if ($action === 'agents') {
    $payload = json_decode((string) stream_get_contents(STDIN), true);
    if (! is_array($payload) || empty($payload['mcp']) || empty($payload['command'])) {
        $emit(['ok' => false, 'error' => 'missing payload']);
        exit(1);
    }
    if (! is_dir($dir)) {
        @mkdir($dir, 0775, true);
    }
    $own($dir);
    file_put_contents($dir . '/.gitignore', "*\n");
    $own($dir . '/.gitignore');
    $writeMcp($payload);
    $server = ['command' => $payload['command'], 'args' => $payload['args']];
    foreach ($jsonFiles as $file => $spec) {
        $path = $base . '/' . $file;
        if (! $spec['always'] && ! is_dir(dirname($path)) && ! is_file($path)) {
            continue;
        }
        $data = is_file($path) ? json_decode((string) file_get_contents($path), true) : [];
        if (! is_array($data)) {
            continue;
        }
        $data[$spec['key']] = ($data[$spec['key']] ?? []) + [];
        $data[$spec['key']]['laravel-openidea'] = $spec['stdio'] ? ['type' => 'stdio'] + $server : $server;
        $writeJson($path, $data);
    }
    $guide = (string) ($payload['guidelines'] ?? '');
    if (is_dir($base . '/vendor/laravel/boost')) {
        if (! is_dir($base . '/.ai/guidelines')) {
            @mkdir($base . '/.ai/guidelines', 0775, true);
            $own($base . '/.ai');
            $own($base . '/.ai/guidelines');
        }
        file_put_contents($base . '/.ai/guidelines/laravel-openidea.md', $guide);
        $own($base . '/.ai/guidelines/laravel-openidea.md');
    }
    $targets = is_dir($base . '/vendor/laravel/boost') ? [] : array_values(array_filter(['AGENTS.md', 'CLAUDE.md', 'GEMINI.md', '.github/copilot-instructions.md', '.junie/guidelines.md'], static fn ($f) => is_file($base . '/' . $f)));
    if ($targets === [] && ! is_dir($base . '/vendor/laravel/boost')) {
        $targets = ['AGENTS.md'];
    }
    foreach ($targets as $f) {
        $path = $base . '/' . $f;
        $text = is_file($path) ? $stripBlock((string) file_get_contents($path)) : '';
        $text = rtrim($text) . ($text === '' ? '' : "\n\n") . $agentMarker[0] . "\n" . trim($guide) . "\n" . $agentMarker[1] . "\n";
        file_put_contents($path, $text);
        $own($path);
    }
    $emit(['ok' => true] + $agentStatus());
    exit(0);
}

if ($action === 'agents-off') {
    foreach ($jsonFiles as $file => $spec) {
        $path = $base . '/' . $file;
        if (! is_file($path)) {
            continue;
        }
        $data = json_decode((string) file_get_contents($path), true);
        if (! is_array($data) || ! isset($data[$spec['key']]['laravel-openidea'])) {
            continue;
        }
        unset($data[$spec['key']]['laravel-openidea']);
        if ($data[$spec['key']] === []) {
            unset($data[$spec['key']]);
        }
        if ($data === []) {
            @unlink($path);
            continue;
        }
        $writeJson($path, $data);
    }
    foreach (['AGENTS.md', 'CLAUDE.md', 'GEMINI.md', '.github/copilot-instructions.md', '.junie/guidelines.md'] as $f) {
        $path = $base . '/' . $f;
        if (is_file($path) && str_contains((string) file_get_contents($path), $agentMarker[0])) {
            $text = rtrim($stripBlock((string) file_get_contents($path))) . "\n";
            trim($text) === '' ? @unlink($path) : file_put_contents($path, $text);
        }
    }
    @unlink($base . '/.ai/guidelines/laravel-openidea.md');
    foreach (['mcp.php', 'install.php', 'recorder.source', 'mcp-access.json'] as $name) {
        @unlink($dir . '/' . $name);
    }
    if (! is_file($file)) {
        @unlink($dir . '/.gitignore');
        @rmdir($dir);
    }
    $emit(['ok' => true] + $agentStatus());
    exit(0);
}

if ($action === 'agents-status') {
    $emit(['ok' => true] + $agentStatus());
    exit(0);
}

if ($action === 'clear') {
    foreach (['', '.1'] as $suffix) {
        @unlink($base . '/storage/logs/openidea-audit.jsonl' . $suffix);
    }
    $emit(['ok' => true]);
    exit(0);
}

$emit(['ok' => true, 'active' => is_file($file) && $hooked()]);
