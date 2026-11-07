<?php

$input = json_decode((string) stream_get_contents(STDIN), true) ?: [];
$deadline = microtime(true) + (float) ($input['seconds'] ?? 3.5);
$results = [];
$base = getcwd();
$env = null;
$tmp = (string) ($input['tmp'] ?? '');
if ($tmp !== '' && ! str_contains($tmp, '..') && ! str_starts_with($tmp, '/')) {
    $tmp = $base . '/' . $tmp;
    @mkdir($tmp, 0775, true);
    if (is_dir($tmp)) {
        $env = getenv();
        $env['TMPDIR'] = $tmp;
    }
}

foreach ($input['jobs'] ?? [] as $job) {
    $result = ['name' => $job['name'] ?? '', 'code' => null, 'stdout' => '', 'stderr' => '', 'timeout' => false];
    $left = $deadline - microtime(true);
    if ($left <= 0.2) {
        $result['timeout'] = true;
        $results[] = $result;
        continue;
    }
    $cmd = array_merge([PHP_BINARY], array_map('strval', $job['argv'] ?? []));
    $proc = proc_open($cmd, [0 => ['pipe', 'r'], 1 => ['pipe', 'w'], 2 => ['pipe', 'w']], $pipes, $base, $env);
    if (! is_resource($proc)) {
        $result['stderr'] = 'could not start ' . ($job['argv'][0] ?? '');
        $results[] = $result;
        continue;
    }
    fclose($pipes[0]);
    stream_set_blocking($pipes[1], false);
    stream_set_blocking($pipes[2], false);
    while (true) {
        $result['stdout'] .= (string) stream_get_contents($pipes[1]);
        $result['stderr'] .= (string) stream_get_contents($pipes[2]);
        $status = proc_get_status($proc);
        if (! $status['running']) {
            $result['code'] = $status['exitcode'];
            break;
        }
        if (microtime(true) >= $deadline) {
            proc_terminate($proc, 9);
            $result['timeout'] = true;
            break;
        }
        usleep(20000);
    }
    $result['stdout'] .= (string) stream_get_contents($pipes[1]);
    $result['stderr'] .= (string) stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);
    proc_close($proc);
    $results[] = $result;
}

$uid = @fileowner($base);
if ($env !== null && $uid !== false && function_exists('posix_geteuid') && posix_geteuid() === 0) {
    $gid = @filegroup($base);
    $items = new RecursiveIteratorIterator(new RecursiveDirectoryIterator($tmp, FilesystemIterator::SKIP_DOTS), RecursiveIteratorIterator::SELF_FIRST);
    foreach ([new SplFileInfo($tmp), ...$items] as $item) {
        if ($item->getOwner() !== $uid) {
            @chown($item->getPathname(), $uid);
            @chgrp($item->getPathname(), $gid);
        }
    }
}

echo "\n@@openidea@@" . json_encode($results, JSON_UNESCAPED_SLASHES | JSON_INVALID_UTF8_SUBSTITUTE) . "\n";
