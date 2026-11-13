<?php

$script = $argv[1] ?? dirname(__DIR__, 2) . '/php/request.php';
$fixture = __DIR__ . '/request';
$failures = 0;

function prepare(string $fixture): string
{
    $dir = sys_get_temp_dir() . '/openidea-request-' . bin2hex(random_bytes(6));
    mkdir($dir . '/vendor', 0777, true);
    mkdir($dir . '/bootstrap');
    copy($fixture . '/vendor/autoload.php', $dir . '/vendor/autoload.php');
    copy($fixture . '/bootstrap/app.php', $dir . '/bootstrap/app.php');
    $pdo = new PDO('sqlite:' . $dir . '/database.sqlite');
    $pdo->exec('create table writes (id integer primary key, note text)');
    return $dir;
}

function cleanup(string $dir): void
{
    foreach (['vendor/autoload.php', 'bootstrap/app.php', 'database.sqlite', 'database.sqlite-journal', 'handled'] as $file) {
        @unlink($dir . '/' . $file);
    }
    @rmdir($dir . '/vendor');
    @rmdir($dir . '/bootstrap');
    @rmdir($dir);
}

function send(string $script, string $dir, array $input, string $fail): array
{
    $env = getenv();
    $env['FIXTURE_FAIL'] = $fail;
    $process = proc_open([PHP_BINARY, $script], [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']], $pipes, $dir, $env);
    fwrite($pipes[0], json_encode($input));
    fclose($pipes[0]);
    $out = stream_get_contents($pipes[1]);
    $err = stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    proc_close($process);
    $at = strrpos($out, '@@openidea-request@@');
    if ($at === false) {
        return ['error' => 'no reply: ' . trim($out . ' ' . $err)];
    }
    return json_decode(trim(substr($out, $at + strlen('@@openidea-request@@'))), true) ?? ['error' => 'unreadable reply'];
}

$cases = [
    ['Rollback on, the transaction starts', true, '', ['handled' => true, 'rows' => 0, 'status' => 200, 'rolledBack' => true, 'queries' => 1]],
    ['Rollback on, beginTransaction() throws', true, 'begin', ['handled' => false, 'rows' => 0, 'error' => 'Nothing was sent']],
    ['Rollback on, beginTransaction() opens no transaction', true, 'begin-silently', ['handled' => false, 'rows' => 0, 'error' => 'Nothing was sent']],
    ['Rollback on, the default connection throws', true, 'connection', ['handled' => false, 'rows' => 0, 'error' => 'Nothing was sent']],
    ['Rollback on, the query listener throws', true, 'listen', ['handled' => true, 'rows' => 0, 'status' => 200, 'rolledBack' => true, 'queries' => null]],
    ['Rollback off', false, '', ['handled' => true, 'rows' => 1, 'status' => 200, 'rolledBack' => false, 'queries' => 1]],
    ['Rollback off, the query listener throws', false, 'listen', ['handled' => true, 'rows' => 1, 'status' => 200, 'rolledBack' => false, 'queries' => null]],
];

foreach ($cases as [$name, $rollback, $fail, $expected]) {
    $dir = prepare($fixture);
    $reply = send($script, $dir, ['method' => 'POST', 'uri' => '/writes', 'rollback' => $rollback, 'json' => true], $fail);
    $actual = [
        'handled' => is_file($dir . '/handled'),
        'rows' => (int) (new PDO('sqlite:' . $dir . '/database.sqlite'))->query('select count(*) from writes')->fetchColumn(),
    ];
    $problems = [];
    foreach ($expected as $key => $value) {
        if ($key === 'handled' || $key === 'rows') {
            if ($actual[$key] !== $value) {
                $problems[] = "$key is " . var_export($actual[$key], true) . ', expected ' . var_export($value, true);
            }
        } elseif ($key === 'error') {
            if (!str_starts_with((string) ($reply['error'] ?? ''), $value)) {
                $problems[] = 'error is ' . var_export($reply['error'] ?? null, true) . ", expected it to start with '$value'";
            }
        } elseif (!array_key_exists($key, $reply) || $reply[$key] !== $value) {
            $problems[] = "$key is " . var_export($reply[$key] ?? null, true) . ', expected ' . var_export($value, true) . (isset($reply['error']) ? ' (error: ' . $reply['error'] . ')' : '');
        }
    }
    gc_collect_cycles();
    cleanup($dir);
    if ($problems === []) {
        echo "ok    $name\n";
    } else {
        $failures++;
        echo "FAIL  $name: " . implode('; ', $problems) . "\n";
    }
}

echo $failures === 0 ? "\nAll " . count($cases) . " cases passed\n" : "\n$failures of " . count($cases) . " cases failed\n";
exit($failures === 0 ? 0 : 1);
