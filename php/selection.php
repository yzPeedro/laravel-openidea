<?php

$base = getcwd();
$dir = $base . '/storage/openidea';
$file = $dir . '/selection.php';

$own = static function (string $path) use ($base): void {
    $uid = @fileowner($base);
    $gid = @filegroup($base);
    if ($uid !== false && function_exists('posix_geteuid') && posix_geteuid() === 0) {
        @chown($path, $uid);
        @chgrp($path, $gid);
    }
};

if (! is_dir($dir) && ! @mkdir($dir, 0775, true) && ! is_dir($dir)) {
    fwrite(STDERR, "cannot create $dir\n");
    exit(1);
}
if (! is_file($dir . '/.gitignore')) {
    file_put_contents($dir . '/.gitignore', "*\n");
    $own($dir);
    $own($dir . '/.gitignore');
}
if (file_put_contents($file, stream_get_contents(STDIN)) === false) {
    fwrite(STDERR, "cannot write $file\n");
    exit(1);
}
$own($file);
echo "ok\n";
