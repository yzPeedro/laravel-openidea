<?php

$__openideaInput = json_decode((string) stream_get_contents(STDIN), true) ?: [];
$__openideaBase = getcwd();
error_reporting(E_ALL & ~E_DEPRECATED & ~E_USER_DEPRECATED);

$__openideaReply = static function (array $payload): void {
    fwrite(STDOUT, "\n@@openidea-request@@" . json_encode($payload, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR) . "\n");
};

try {
    require $__openideaBase . '/vendor/autoload.php';
    $app = require $__openideaBase . '/bootstrap/app.php';
    $kernel = $app->make(Illuminate\Contracts\Http\Kernel::class);
    $kernel->bootstrap();
} catch (Throwable $e) {
    $__openideaReply(['error' => 'The app did not boot: ' . $e->getMessage()]);
    exit(0);
}

$passThrough = new class {
    public function handle($request, $next)
    {
        return $next($request);
    }
};
foreach ([
    'Illuminate\Foundation\Http\Middleware\VerifyCsrfToken',
    'Illuminate\Foundation\Http\Middleware\ValidateCsrfToken',
    'Illuminate\Foundation\Http\Middleware\PreventRequestForgery',
    'App\Http\Middleware\VerifyCsrfToken',
] as $csrf) {
    $app->instance($csrf, $passThrough);
}

$method = strtoupper((string) ($__openideaInput['method'] ?? 'GET'));
$uri = '/' . ltrim((string) ($__openideaInput['uri'] ?? '/'), '/');
$query = ltrim((string) ($__openideaInput['query'] ?? ''), '?');
$body = $__openideaInput['body'] ?? null;
$json = (bool) ($__openideaInput['json'] ?? true);
$rollback = (bool) ($__openideaInput['rollback'] ?? true);
$userId = $__openideaInput['user'] ?? null;

$server = [
    'HTTP_ACCEPT' => $json ? 'application/json' : 'text/html,application/xhtml+xml',
    'REMOTE_ADDR' => '127.0.0.1',
    'HTTP_USER_AGENT' => 'Laravel OpenIDEA',
];
$content = null;
$parameters = [];
if (is_array($body) && $method !== 'GET' && $method !== 'HEAD') {
    if ($json) {
        $server['CONTENT_TYPE'] = 'application/json';
        $content = json_encode($body);
    } else {
        $parameters = $body;
    }
}

$queries = 0;
$connection = null;
try {
    $connection = $app['db']->connection();
    $connection->listen(static function () use (&$queries): void {
        $queries++;
    });
    if ($rollback) {
        $connection->beginTransaction();
    }
} catch (Throwable $e) {
    $connection = null;
}

$finish = static function () use (&$connection, $rollback): void {
    if ($connection !== null && $rollback) {
        while ($connection->transactionLevel() > 0) {
            $connection->rollBack();
        }
    }
};

try {
    $request = Illuminate\Http\Request::create($uri . ($query !== '' ? '?' . $query : ''), $method, $parameters, [], [], $server, $content);
    $app->instance('request', $request);
    if ($userId !== null && $userId !== '') {
        $model = config('auth.providers.users.model');
        $user = is_string($model) && class_exists($model) ? $model::find($userId) : null;
        if ($user === null) {
            $finish();
            $__openideaReply(['error' => "No user with id $userId in " . (is_string($model) ? $model : 'the users provider') . '.']);
            exit(0);
        }
        $app['auth']->guard()->setUser($user);
    }

    $started = microtime(true);
    $response = $kernel->handle($request);
    $ms = round((microtime(true) - $started) * 1000, 1);

    if ($response instanceof Symfony\Component\HttpFoundation\BinaryFileResponse) {
        $text = '(file ' . $response->getFile()->getFilename() . ', ' . $response->getFile()->getSize() . ' bytes)';
    } elseif ($response instanceof Symfony\Component\HttpFoundation\StreamedResponse) {
        ob_start();
        $response->sendContent();
        $text = (string) ob_get_clean();
    } else {
        $text = (string) $response->getContent();
    }
    $size = strlen($text);
    if ($size > 200000) {
        $text = substr($text, 0, 200000) . "\n… cut at 200 KB of " . $size . ' bytes';
    }

    $headers = [];
    foreach ($response->headers->all() as $name => $values) {
        $headers[$name] = strtolower($name) === 'set-cookie' ? '•••' : implode(', ', $values);
    }

    $kernel->terminate($request, $response);
    $finish();

    $__openideaReply([
        'status' => $response->getStatusCode(),
        'reason' => Symfony\Component\HttpFoundation\Response::$statusTexts[$response->getStatusCode()] ?? '',
        'headers' => $headers,
        'body' => $text,
        'size' => $size,
        'ms' => $ms,
        'queries' => $queries,
        'redirect' => $response->headers->get('Location'),
        'rolledBack' => $rollback && $connection !== null,
        'user' => $userId,
    ]);
} catch (Throwable $e) {
    $finish();
    $__openideaReply(['error' => get_class($e) . ': ' . $e->getMessage()]);
}
