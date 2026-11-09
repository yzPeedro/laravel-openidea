<?php

namespace OpenIdea\Recorder;

use Illuminate\Support\ServiceProvider;
use Throwable;

final class Recorder
{
    private const MAX_BYTES = 8_000_000;
    private const SECRET = '/pass(word)?|secret|token|api[_-]?key|authorization|cookie|credential|private|signature|otp|cvv|card|(^|[_.-])key$/i';
    private const PII = '/e-?mail|cpf|cnpj|rg$|document|phone|telefone|celular|mobile|birth|nascimento|address|endereco|cep|zip|ssn|passport/i';

    public static bool $showPii = false;

    private string $id;
    private float $started;
    private int $seq = 0;

    public function __construct(private readonly string $path)
    {
        $this->id = bin2hex(random_bytes(6));
        $this->started = microtime(true);
    }

    public function id(): string
    {
        return $this->id;
    }

    public function newScope(): void
    {
        $this->id = bin2hex(random_bytes(6));
        $this->started = microtime(true);
    }

    public function write(string $type, array $data): void
    {
        try {
            $record = ['t' => round(microtime(true), 4), 'seq' => ++$this->seq, 'scope' => $this->id, 'pid' => getmypid(), 'type' => $type] + $data;
            $line = json_encode($record, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_INVALID_UTF8_SUBSTITUTE | JSON_PARTIAL_OUTPUT_ON_ERROR);
            if ($line === false) {
                return;
            }
            if (is_file($this->path) && filesize($this->path) > self::MAX_BYTES) {
                @rename($this->path, $this->path . '.1');
            }
            $fresh = ! is_file($this->path);
            @file_put_contents($this->path, $line . "\n", FILE_APPEND | LOCK_EX);
            if ($fresh && function_exists('posix_geteuid') && posix_geteuid() === 0) {
                $owner = @fileowner(dirname($this->path));
                if ($owner !== false) {
                    @chown($this->path, $owner);
                    @chgrp($this->path, (int) @filegroup(dirname($this->path)));
                }
            }
        } catch (Throwable) {
        }
    }

    public function elapsed(): float
    {
        return round((microtime(true) - $this->started) * 1000, 2);
    }

    public static function caller(int $skip = 0): array
    {
        foreach (debug_backtrace(DEBUG_BACKTRACE_IGNORE_ARGS, 40) as $frame) {
            $file = $frame['file'] ?? '';
            if ($file === '' || str_contains($file, 'Command line code') || str_contains($file, 'Standard input code') || str_contains($file, "eval()'d code") || str_contains($file, '/vendor/') || str_contains($file, '\\vendor\\') || str_contains($file, 'storage/openidea') || str_contains($file, 'storage\\openidea')) {
                continue;
            }
            if ($skip-- > 0) {
                continue;
            }
            return ['file' => $file, 'line' => $frame['line'] ?? 0];
        }
        return ['file' => null, 'line' => null];
    }

    public static function value(mixed $value, int $depth = 0, ?string $key = null): mixed
    {
        if ($key !== null && (preg_match(self::SECRET, $key) || (! self::$showPii && preg_match(self::PII, $key)))) {
            return $value === null ? null : '•••';
        }
        if ($value === null || is_bool($value) || is_int($value) || is_float($value)) {
            return $value;
        }
        if (is_string($value)) {
            return mb_strlen($value) > 400 ? mb_substr($value, 0, 400) . '…' : $value;
        }
        if ($depth >= 3) {
            return is_object($value) ? '{' . get_class($value) . '}' : (is_array($value) ? '[' . count($value) . ' items]' : get_debug_type($value));
        }
        if (is_array($value)) {
            $out = [];
            $i = 0;
            foreach ($value as $k => $v) {
                if ($i++ >= 25) {
                    $out['…'] = (count($value) - 25) . ' more';
                    break;
                }
                $out[$k] = self::value($v, $depth + 1, is_string($k) ? $k : null);
            }
            return $out;
        }
        if ($value instanceof \Illuminate\Database\Eloquent\Model) {
            $attributes = [];
            foreach (array_slice($value->getAttributes(), 0, 30, true) as $k => $v) {
                $attributes[$k] = in_array($k, $value->getHidden(), true) ? '•••' : self::value($v, $depth + 1, $k);
            }
            return ['@model' => get_class($value), '@key' => $value->getKey(), '@exists' => $value->exists, '@dirty' => array_keys($value->getDirty())] + $attributes;
        }
        if ($value instanceof \Illuminate\Support\Collection) {
            return ['@collection' => get_class($value), '@count' => $value->count()] + (array) self::value($value->take(10)->all(), $depth + 1);
        }
        if ($value instanceof \UnitEnum) {
            return get_class($value) . '::' . $value->name;
        }
        if ($value instanceof \DateTimeInterface) {
            return $value->format('Y-m-d H:i:s.u P');
        }
        if ($value instanceof \Closure) {
            return '{Closure}';
        }
        if (is_object($value)) {
            $out = ['@class' => get_class($value)];
            foreach (array_slice(get_object_vars($value), 0, 25, true) as $k => $v) {
                $out[$k] = self::value($v, $depth + 1, $k);
            }
            return $out;
        }
        return get_debug_type($value);
    }

    public static function listener(mixed $listener): ?string
    {
        if (is_string($listener)) {
            return $listener;
        }
        if (is_array($listener) && count($listener) === 2) {
            return (is_object($listener[0]) ? get_class($listener[0]) : (string) $listener[0]) . '@' . $listener[1];
        }
        if ($listener instanceof \Closure) {
            try {
                $r = new \ReflectionFunction($listener);
                $file = $r->getFileName() ?: '';
                if (str_contains($file, 'storage/openidea') || str_contains($file, 'storage\\openidea')) {
                    return null;
                }
                if (str_contains($file, '/vendor/laravel/framework/src/Illuminate/Events/')) {
                    return 'Closure (framework)';
                }
                return 'Closure ' . basename($file) . ':' . $r->getStartLine();
            } catch (Throwable) {
                return 'Closure';
            }
        }
        if (is_object($listener)) {
            return get_class($listener);
        }
        return get_debug_type($listener);
    }
}

final class RecorderServiceProvider extends ServiceProvider
{
    private const SKIP = [
        'Illuminate\Database\Events\QueryExecuted',
        'Illuminate\Database\Events\StatementPrepared',
        'Illuminate\Database\Events\ConnectionEstablished',
        'Illuminate\Log\Events\MessageLogged',
        'Illuminate\Foundation\Http\Events\RequestHandled',
        'Illuminate\Console\Events\ArtisanStarting',
        'Illuminate\Foundation\Events\LocaleUpdated',
        'Illuminate\Cache\Events\RetrievingKey',
        'Illuminate\Cache\Events\RetrievingManyKeys',
        'Illuminate\Cache\Events\WritingKey',
        'Illuminate\Cache\Events\WritingManyKeys',
        'Illuminate\Routing\Events\Routing',
        'Illuminate\Routing\Events\PreparingResponse',
        'Illuminate\Routing\Events\ResponsePrepared',
        'Illuminate\Cache\Events\CacheHit',
        'Illuminate\Cache\Events\CacheMissed',
        'Illuminate\Cache\Events\KeyWritten',
        'Illuminate\Cache\Events\KeyForgotten',
        'Illuminate\Queue\Events\JobProcessing',
        'Illuminate\Queue\Events\JobProcessed',
        'Illuminate\Queue\Events\JobFailed',
        'Illuminate\Queue\Events\JobQueued',
        'Illuminate\Queue\Events\JobQueueing',
        'Illuminate\Mail\Events\MessageSent',
        'Illuminate\Notifications\Events\NotificationSent',
        'Illuminate\Auth\Access\Events\GateEvaluated',
        'Illuminate\Console\Events\CommandStarting',
        'Illuminate\Console\Events\CommandFinished',
    ];
    private const SKIP_PREFIX = ['eloquent.booting', 'eloquent.booted', 'eloquent.retrieved', 'bootstrapping:', 'bootstrapped:', 'composing:', 'creating:', 'Illuminate\Http\Client\Events\\'];

    public function register(): void
    {
        if (! $this->enabled()) {
            return;
        }
        $recorder = new Recorder(storage_path('logs/openidea-audit.jsonl'));
        $this->app->instance(Recorder::class, $recorder);
    }

    private function enabled(): bool
    {
        $flag = env('OPENIDEA_AUDIT');
        if ($flag === false || $flag === 'false' || $flag === '0') {
            return false;
        }
        return $this->app->environment('local', 'development', 'dev') || $flag === true || $flag === 'true';
    }

    public function boot(): void
    {
        if (! $this->app->bound(Recorder::class)) {
            return;
        }
        $rec = $this->app->make(Recorder::class);
        $pii = env('OPENIDEA_AUDIT_SHOW_PII');
        Recorder::$showPii = $pii === true || $pii === 'true' || $pii === '1';
        $events = $this->app['events'];
        $context = $this->app->runningInConsole() ? ['context' => 'console', 'argv' => implode(' ', array_slice($_SERVER['argv'] ?? [], 0, 8))] : ['context' => 'http'];
        $rec->write('boot', $context + ['php' => PHP_VERSION, 'laravel' => $this->app->version(), 'env' => $this->app->environment()]);

        $events->listen('*', function (string $name, array $payload) use ($rec, $events) {
            if (in_array($name, self::SKIP, true)) {
                return;
            }
            foreach (self::SKIP_PREFIX as $prefix) {
                if (str_starts_with($name, $prefix)) {
                    return;
                }
            }
            $listeners = [];
            try {
                $raw = method_exists($events, 'getRawListeners') ? $events->getRawListeners() : [];
                foreach ($raw[$name] ?? [] as $listener) {
                    $label = Recorder::listener($listener);
                    if ($label !== null) {
                        $listeners[] = $label;
                    }
                }
            } catch (Throwable) {
            }
            $event = $payload[0] ?? null;
            $data = is_object($event) && ! str_starts_with($name, 'eloquent.') ? Recorder::value($event) : Recorder::value($payload);
            $rec->write('event', ['name' => $name, 'listeners' => $listeners, 'payload' => $data] + Recorder::caller());
        });

        $events->listen(\Illuminate\Database\Events\QueryExecuted::class, function ($query) use ($rec) {
            $bindings = [];
            foreach ($query->bindings as $i => $b) {
                $bindings[$i] = $b instanceof \DateTimeInterface ? $b->format('Y-m-d H:i:s') : (is_scalar($b) || $b === null ? (is_string($b) && mb_strlen($b) > 200 ? mb_substr($b, 0, 200) . '…' : $b) : get_debug_type($b));
            }
            $rec->write('query', ['sql' => $query->sql, 'bindings' => $bindings, 'ms' => $query->time, 'connection' => $query->connectionName] + Recorder::caller());
        });

        $events->listen(\Illuminate\Log\Events\MessageLogged::class, function ($log) use ($rec) {
            $exception = $log->context['exception'] ?? null;
            $data = ['level' => $log->level, 'message' => mb_substr((string) $log->message, 0, 2000)];
            if ($exception instanceof Throwable) {
                $frames = [];
                foreach (array_slice($exception->getTrace(), 0, 15) as $f) {
                    if (isset($f['file'])) {
                        $frames[] = ['file' => $f['file'], 'line' => $f['line'] ?? 0, 'call' => ($f['class'] ?? '') . ($f['type'] ?? '') . ($f['function'] ?? '')];
                    }
                }
                $rec->write('exception', $data + ['class' => get_class($exception), 'file' => $exception->getFile(), 'line' => $exception->getLine(), 'trace' => $frames]);
                return;
            }
            $context = $log->context;
            unset($context['exception']);
            $rec->write('log', $data + ['context' => Recorder::value($context)] + Recorder::caller());
        });

        $events->listen(\Illuminate\Foundation\Http\Events\RequestHandled::class, function ($e) use ($rec) {
            $request = $e->request;
            $route = $request->route();
            $input = [];
            foreach (array_slice($request->all(), 0, 30, true) as $k => $v) {
                $input[$k] = Recorder::value($v, 1, (string) $k);
            }
            $headers = [];
            foreach (['accept', 'content-type', 'user-agent', 'referer', 'x-requested-with'] as $h) {
                if ($request->headers->has($h)) {
                    $headers[$h] = mb_substr((string) $request->headers->get($h), 0, 200);
                }
            }
            $user = null;
            try {
                $user = $request->user()?->getAuthIdentifier();
            } catch (Throwable) {
            }
            $rec->write('request', [
                'method' => $request->getMethod(),
                'uri' => '/' . ltrim($request->path(), '/'),
                'query' => $request->getQueryString(),
                'status' => $e->response->getStatusCode(),
                'ms' => $rec->elapsed(),
                'route' => $route?->getName(),
                'action' => $route?->getActionName(),
                'middleware' => $route ? array_slice($route->gatherMiddleware(), 0, 20) : [],
                'ip' => $request->ip(),
                'user' => $user,
                'input' => $input,
                'headers' => $headers,
                'memory' => memory_get_peak_usage(true),
            ]);
        });

        $events->listen(\Illuminate\Console\Events\CommandFinished::class, function ($e) use ($rec) {
            if ($e->command === null || in_array($e->command, ['schedule:run', 'schedule:work', 'queue:work', 'horizon:work'], true)) {
                return;
            }
            $rec->write('command', ['command' => $e->command, 'exit' => $e->exitCode, 'ms' => $rec->elapsed()]);
        });

        $events->listen(\Illuminate\Queue\Events\JobProcessing::class, function ($e) use ($rec) {
            $rec->newScope();
            $rec->write('job', ['status' => 'processing', 'job' => $e->job->resolveName(), 'queue' => $e->job->getQueue(), 'connection' => $e->connectionName, 'attempt' => $e->job->attempts(), 'id' => $e->job->getJobId()]);
        });
        $events->listen(\Illuminate\Queue\Events\JobProcessed::class, function ($e) use ($rec) {
            $rec->write('job', ['status' => 'processed', 'job' => $e->job->resolveName(), 'queue' => $e->job->getQueue(), 'connection' => $e->connectionName, 'ms' => $rec->elapsed(), 'id' => $e->job->getJobId()]);
        });
        $events->listen(\Illuminate\Queue\Events\JobFailed::class, function ($e) use ($rec) {
            $rec->write('job', ['status' => 'failed', 'job' => $e->job->resolveName(), 'queue' => $e->job->getQueue(), 'connection' => $e->connectionName, 'ms' => $rec->elapsed(), 'error' => get_class($e->exception) . ': ' . $e->exception->getMessage(), 'id' => $e->job->getJobId()]);
        });
        $events->listen(\Illuminate\Queue\Events\JobQueued::class, function ($e) use ($rec) {
            $job = is_object($e->job) ? get_class($e->job) : (string) $e->job;
            $rec->write('job', ['status' => 'queued', 'job' => $job, 'queue' => property_exists($e, 'queue') ? $e->queue : null, 'connection' => $e->connectionName, 'id' => $e->id, 'payload' => is_object($e->job) ? Recorder::value($e->job) : null] + Recorder::caller());
        });

        $events->listen(\Illuminate\Cache\Events\CacheHit::class, fn ($e) => $rec->write('cache', ['op' => 'hit', 'key' => $e->key, 'store' => $e->storeName ?? null] + Recorder::caller()));
        $events->listen(\Illuminate\Cache\Events\CacheMissed::class, fn ($e) => $rec->write('cache', ['op' => 'miss', 'key' => $e->key, 'store' => $e->storeName ?? null] + Recorder::caller()));
        $events->listen(\Illuminate\Cache\Events\KeyWritten::class, fn ($e) => $rec->write('cache', ['op' => 'write', 'key' => $e->key, 'store' => $e->storeName ?? null, 'ttl' => $e->seconds ?? null] + Recorder::caller()));
        $events->listen(\Illuminate\Cache\Events\KeyForgotten::class, fn ($e) => $rec->write('cache', ['op' => 'forget', 'key' => $e->key, 'store' => $e->storeName ?? null] + Recorder::caller()));

        $events->listen(\Illuminate\Mail\Events\MessageSent::class, function ($e) use ($rec) {
            $message = $e->message ?? null;
            $to = [];
            try {
                foreach ($message->getTo() as $address) {
                    $to[] = $address->getAddress();
                }
            } catch (Throwable) {
            }
            $rec->write('mail', ['subject' => method_exists($message, 'getSubject') ? $message->getSubject() : null, 'to' => count($to) . ' recipient(s)', 'mailable' => $e->data['__laravel_mailable'] ?? null]);
        });
        $events->listen(\Illuminate\Notifications\Events\NotificationSent::class, function ($e) use ($rec) {
            $rec->write('notification', ['notification' => get_class($e->notification), 'channel' => $e->channel, 'notifiable' => is_object($e->notifiable) ? get_class($e->notifiable) . (method_exists($e->notifiable, 'getKey') ? '#' . $e->notifiable->getKey() : '') : null]);
        });
        $events->listen(\Illuminate\Http\Client\Events\ResponseReceived::class, function ($e) use ($rec) {
            $rec->write('http', ['method' => $e->request->method(), 'url' => preg_replace('/([?&](?:token|key|secret|signature|password)=)[^&]+/i', '$1•••', $e->request->url()), 'status' => $e->response->status(), 'ms' => round(($e->response->transferStats?->getTransferTime() ?? 0) * 1000, 2)] + Recorder::caller());
        });
        $events->listen(\Illuminate\Auth\Access\Events\GateEvaluated::class, function ($e) use ($rec) {
            $rec->write('gate', ['ability' => $e->ability, 'result' => $e->result instanceof \Illuminate\Auth\Access\Response ? $e->result->allowed() : (bool) $e->result, 'user' => $e->user?->getAuthIdentifier()] + Recorder::caller());
        });

        if (class_exists(\Symfony\Component\VarDumper\VarDumper::class)) {
            $previous = null;
            $previous = \Symfony\Component\VarDumper\VarDumper::setHandler(function ($value) use ($rec, &$previous) {
                try {
                    $cloner = new \Symfony\Component\VarDumper\Cloner\VarCloner();
                    $dumper = new \Symfony\Component\VarDumper\Dumper\CliDumper();
                    $dumper->setColors(false);
                    $text = $dumper->dump($cloner->cloneVar($value), true);
                    $rec->write('dump', ['value' => mb_substr((string) $text, 0, 20000)] + Recorder::caller());
                } catch (Throwable) {
                }
                if ($previous) {
                    $previous($value);
                } else {
                    $dumper = in_array(PHP_SAPI, ['cli', 'phpdbg'], true) ? new \Symfony\Component\VarDumper\Dumper\CliDumper() : new \Symfony\Component\VarDumper\Dumper\HtmlDumper();
                    $dumper->dump((new \Symfony\Component\VarDumper\Cloner\VarCloner())->cloneVar($value));
                }
            });
        }
    }
}

spl_autoload_register(static function (): void {
    static $attached = false;
    if ($attached || ! class_exists(\Illuminate\Container\Container::class, false) || ! class_exists(\Illuminate\Foundation\Application::class, false)) {
        return;
    }
    $app = \Closure::bind(static fn () => static::$instance, null, \Illuminate\Container\Container::class)();
    if (! $app instanceof \Illuminate\Foundation\Application) {
        return;
    }
    $attached = true;
    $app->booted(static function ($app): void {
        $provider = new RecorderServiceProvider($app);
        $provider->register();
        $provider->boot();
    });
}, true, true);
