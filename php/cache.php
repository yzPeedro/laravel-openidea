<?php

$__openideaInput = json_decode((string) stream_get_contents(STDIN), true) ?: [];
$__openideaBase = getcwd();
error_reporting(E_ALL & ~E_DEPRECATED & ~E_USER_DEPRECATED);

$__openideaReply = static function (array $payload): void {
    fwrite(STDOUT, "\n@@openidea-cache@@" . json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR) . "\n");
};

try {
    require $__openideaBase . '/vendor/autoload.php';
    $app = require $__openideaBase . '/bootstrap/app.php';
    $app->make(Illuminate\Contracts\Console\Kernel::class)->bootstrap();
} catch (Throwable $e) {
    $__openideaReply(['error' => 'The app did not boot: ' . $e->getMessage()]);
    exit(0);
}

$action = (string) ($__openideaInput['action'] ?? 'list');
$size = max(10, min(500, (int) ($__openideaInput['size'] ?? 50)));
$page = max(0, (int) ($__openideaInput['page'] ?? 0));
$search = strtolower(trim((string) ($__openideaInput['search'] ?? '')));
$wanted = (string) ($__openideaInput['key'] ?? '');
$limit = 20000;
$preview = 300;

$stores = [];
foreach ((array) config('cache.stores', []) as $name => $conf) {
    $driver = (string) ($conf['driver'] ?? '');
    $stores[] = [
        'name' => (string) $name,
        'driver' => $driver,
        'default' => $name === config('cache.default'),
        'listable' => in_array($driver, ['file', 'database', 'redis'], true),
    ];
}
$storeName = (string) ($__openideaInput['store'] ?? '');
if ($storeName === '' || ! collect($stores)->contains('name', $storeName)) {
    $storeName = (string) config('cache.default');
}
$conf = (array) config("cache.stores.$storeName", []);
$driver = (string) ($conf['driver'] ?? '');
$cachePrefix = (string) ($conf['prefix'] ?? config('cache.prefix', ''));

$decode = static function ($raw) {
    if (! is_string($raw)) {
        return $raw;
    }
    if (is_numeric($raw)) {
        return $raw + 0;
    }
    foreach ([$raw, base64_decode($raw, true)] as $candidate) {
        if (! is_string($candidate)) {
            continue;
        }
        $value = @unserialize($candidate, ['allowed_classes' => false]);
        if ($value !== false || $candidate === 'b:0;') {
            return $value;
        }
    }
    return $raw;
};

$plain = static function ($value) use (&$plain) {
    if ($value instanceof __PHP_Incomplete_Class) {
        $props = (array) $value;
        $class = $props['__PHP_Incomplete_Class_Name'] ?? 'object';
        unset($props['__PHP_Incomplete_Class_Name']);
        $out = ['@class' => $class];
        foreach ($props as $k => $v) {
            $out[preg_replace('/^\0[^\0]+\0/', '', (string) $k)] = $plain($v);
        }
        return $out;
    }
    if (is_array($value)) {
        return array_map($plain, $value);
    }
    if (is_object($value)) {
        return ['@class' => get_class($value)];
    }
    return $value;
};

$describe = static function ($value) use ($plain): array {
    $type = get_debug_type($value);
    if ($value instanceof __PHP_Incomplete_Class) {
        $type = ((array) $value)['__PHP_Incomplete_Class_Name'] ?? 'object';
    }
    $text = is_string($value) ? $value : json_encode($plain($value), JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR);
    return [$type, (string) $text];
};

$entries = [];
$total = 0;
$truncated = false;
$error = null;
$note = null;

try {
    if ($driver === 'file') {
        $path = (string) ($conf['path'] ?? storage_path('framework/cache/data'));
        $note = 'The file store keeps only a hash of each key, so keys show as their hash.';
        $files = [];
        if (is_dir($path)) {
            $it = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($path, FilesystemIterator::SKIP_DOTS));
            foreach ($it as $file) {
                if ($file->isFile() && $file->getFilename() !== '.gitignore') {
                    $files[] = $file->getPathname();
                    if (count($files) >= $limit) {
                        $truncated = true;
                        break;
                    }
                }
            }
        }
        sort($files);
        $rows = [];
        foreach ($files as $file) {
            $key = basename($file);
            if ($search !== '' && ! str_contains(strtolower($key), $search)) {
                continue;
            }
            $rows[] = [$key, $file];
        }
        if ($action === 'forget') {
            foreach ($rows as [$key, $file]) {
                if ($key === $wanted) {
                    @unlink($file);
                }
            }
        } else {
            $total = count($rows);
            $slice = $action === 'value' ? array_values(array_filter($rows, fn ($r) => $r[0] === $wanted)) : array_slice($rows, $page * $size, $size);
            foreach ($slice as [$key, $file]) {
                $raw = (string) @file_get_contents($file);
                $expires = (int) substr($raw, 0, 10);
                [$type, $text] = $describe($decode(substr($raw, 10)));
                $entries[] = ['key' => $key, 'type' => $type, 'value' => $text, 'bytes' => strlen($raw), 'expires' => $expires >= 9999999999 ? null : $expires];
            }
        }
    } elseif ($driver === 'database') {
        $db = Illuminate\Support\Facades\DB::connection($conf['connection'] ?? null);
        $table = (string) ($conf['table'] ?? 'cache');
        $query = $db->table($table);
        if ($search !== '') {
            $query->whereRaw('LOWER(' . $db->getQueryGrammar()->wrap('key') . ') LIKE ?', ['%' . $search . '%']);
        }
        if ($action === 'forget') {
            $db->table($table)->where('key', $wanted)->delete();
        } else {
            $total = (clone $query)->count();
            $rows = $action === 'value' ? $db->table($table)->where('key', $wanted)->get() : $query->orderBy('key')->offset($page * $size)->limit($size)->get();
            foreach ($rows as $row) {
                $raw = (string) $row->value;
                [$type, $text] = $describe($decode($raw));
                $entries[] = ['key' => (string) $row->key, 'type' => $type, 'value' => $text, 'bytes' => strlen($raw), 'expires' => (int) $row->expiration];
            }
        }
    } elseif ($driver === 'redis') {
        $connection = Illuminate\Support\Facades\Redis::connection($conf['connection'] ?? 'cache');
        $client = $connection->client();
        $clientPrefix = '';
        if (class_exists(Redis::class, false) && $client instanceof Redis) {
            $clientPrefix = (string) $client->getOption(Redis::OPT_PREFIX);
        } elseif (method_exists($client, 'getOptions')) {
            $prefixOption = $client->getOptions()->prefix ?? null;
            $clientPrefix = $prefixOption ? (string) $prefixOption->getPrefix() : '';
        }
        $prefix = $clientPrefix . $cachePrefix;
        if ($action === 'forget') {
            $connection->command('del', [$cachePrefix . $wanted]);
        } else {
            $keys = [];
            $cursor = null;
            do {
                $result = $client instanceof Redis
                    ? $client->scan($cursor, $prefix . '*', 1000)
                    : $connection->command('scan', [$cursor ?? 0, ['match' => $prefix . '*', 'count' => 1000]]);
                if ($client instanceof Redis) {
                    $batch = $result ?: [];
                } else {
                    [$cursor, $batch] = $result;
                    $cursor = (int) $cursor === 0 ? 0 : $cursor;
                }
                foreach ($batch as $full) {
                    $key = str_starts_with($full, $prefix) ? substr($full, strlen($prefix)) : (str_starts_with($full, $cachePrefix) ? substr($full, strlen($cachePrefix)) : $full);
                    if ($search === '' || str_contains(strtolower($key), $search)) {
                        $keys[] = $key;
                    }
                }
                if (count($keys) >= $limit) {
                    $truncated = true;
                    break;
                }
            } while ($cursor);
            sort($keys);
            $keys = array_values(array_unique($keys));
            $total = count($keys);
            $slice = $action === 'value' ? [$wanted] : array_slice($keys, $page * $size, $size);
            foreach ($slice as $key) {
                $raw = $connection->command('get', [$cachePrefix . $key]);
                $ttl = (int) $connection->command('ttl', [$cachePrefix . $key]);
                [$type, $text] = $describe($decode($raw));
                $entries[] = ['key' => $key, 'type' => $raw === null || $raw === false ? 'not a string value' : $type, 'value' => $text, 'bytes' => is_string($raw) ? strlen($raw) : 0, 'expires' => $ttl > 0 ? time() + $ttl : null];
            }
        }
    } elseif ($driver === 'array') {
        $note = 'The array store lives only for one request or command, so there is nothing to list.';
    } else {
        $note = "Listing keys isn't possible with the $driver driver.";
    }
    if ($action === 'flush') {
        Illuminate\Support\Facades\Cache::store($storeName)->flush();
        $entries = [];
        $total = 0;
    }
} catch (Throwable $e) {
    $error = $e->getMessage();
}

if ($action !== 'value') {
    foreach ($entries as &$entry) {
        $entry['full'] = mb_strlen($entry['value']) <= $preview;
        if (! $entry['full']) {
            $entry['value'] = mb_substr($entry['value'], 0, $preview) . '…';
        }
    }
    unset($entry);
}

$__openideaReply([
    'action' => $action,
    'stores' => $stores,
    'store' => $storeName,
    'driver' => $driver,
    'page' => $page,
    'size' => $size,
    'total' => $total,
    'truncated' => $truncated,
    'entries' => $entries,
    'note' => $note,
    'error' => $error,
    'now' => time(),
]);
