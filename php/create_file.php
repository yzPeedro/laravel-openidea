<?php

$base = getcwd();
$rel = $argv[1] ?? '';
if ($rel === '' || str_contains($rel, '..') || str_starts_with($rel, '/')) {
    fwrite(STDERR, "invalid path\n");
    exit(1);
}
$file = $base . '/' . $rel;
if (file_exists($file)) {
    fwrite(STDERR, "$rel already exists\n");
    exit(2);
}

$own = static function (string $path) use ($base): void {
    $uid = @fileowner($base);
    $gid = @filegroup($base);
    if ($uid !== false && function_exists('posix_geteuid') && posix_geteuid() === 0) {
        @chown($path, $uid);
        @chgrp($path, $gid);
    }
};

$dir = dirname($file);
$made = [];
for ($d = $dir; ! is_dir($d) && $d !== $base && $d !== dirname($d); $d = dirname($d)) {
    $made[] = $d;
}
if (! is_dir($dir) && ! @mkdir($dir, 0775, true) && ! is_dir($dir)) {
    fwrite(STDERR, "cannot create $dir\n");
    exit(1);
}
foreach (array_reverse($made) as $d) {
    $own($d);
}
if (file_put_contents($file, stream_get_contents(STDIN)) === false) {
    fwrite(STDERR, "cannot write $rel\n");
    exit(1);
}
$own($file);
echo "ok\n";
