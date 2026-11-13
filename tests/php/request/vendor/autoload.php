<?php

namespace Illuminate\Contracts\Http {
    interface Kernel
    {
    }
}

namespace Illuminate\Http {
    class Request
    {
        public string $uri = '';
        public string $method = 'GET';

        public static function create($uri, $method = 'GET', $parameters = [], $cookies = [], $files = [], $server = [], $content = null): self
        {
            $request = new self();
            $request->uri = $uri;
            $request->method = $method;
            return $request;
        }
    }
}

namespace Symfony\Component\HttpFoundation {
    class HeaderBag
    {
        public function __construct(private array $headers = [])
        {
        }

        public function all(): array
        {
            return $this->headers;
        }

        public function get(string $name): ?string
        {
            return $this->headers[strtolower($name)][0] ?? null;
        }
    }

    class Response
    {
        public static array $statusTexts = [200 => 'OK'];
        public HeaderBag $headers;

        public function __construct(private string $content = '', private int $status = 200)
        {
            $this->headers = new HeaderBag(['content-type' => ['text/plain']]);
        }

        public function getContent(): string
        {
            return $this->content;
        }

        public function getStatusCode(): int
        {
            return $this->status;
        }
    }
}

namespace OpenideaFixture {
    use RuntimeException;
    use Symfony\Component\HttpFoundation\Response;

    function fail(string $step): bool
    {
        return in_array($step, explode(',', (string) getenv('FIXTURE_FAIL')), true);
    }

    class Connection
    {
        private array $listeners = [];
        private int $level = 0;

        public function __construct(private \PDO $pdo)
        {
        }

        public function beginTransaction(): void
        {
            if (fail('begin')) {
                throw new RuntimeException('beginTransaction failed');
            }
            if (fail('begin-silently')) {
                return;
            }
            $this->pdo->beginTransaction();
            $this->level++;
        }

        public function transactionLevel(): int
        {
            return $this->level;
        }

        public function rollBack(): void
        {
            $this->pdo->rollBack();
            $this->level--;
        }

        public function listen(callable $listener): void
        {
            if (fail('listen')) {
                throw new RuntimeException('listen failed');
            }
            $this->listeners[] = $listener;
        }

        public function insert(string $sql): void
        {
            $this->pdo->exec($sql);
            foreach ($this->listeners as $listener) {
                $listener();
            }
        }
    }

    class Database
    {
        private ?Connection $connection = null;

        public function __construct(private string $base)
        {
        }

        public function connection(): Connection
        {
            if (fail('connection')) {
                throw new RuntimeException('connection failed');
            }
            return $this->connection ??= new Connection(new \PDO('sqlite:' . $this->base . '/database.sqlite', null, null, [\PDO::ATTR_ERRMODE => \PDO::ERRMODE_EXCEPTION]));
        }
    }

    class Kernel implements \Illuminate\Contracts\Http\Kernel
    {
        public function __construct(private App $app)
        {
        }

        public function bootstrap(): void
        {
        }

        public function handle($request): Response
        {
            file_put_contents($this->app->base . '/handled', $request->method . ' ' . $request->uri);
            $this->app['db']->connection()->insert("insert into writes (note) values ('from the handler')");
            return new Response('written');
        }

        public function terminate($request, $response): void
        {
        }
    }

    class App implements \ArrayAccess
    {
        private array $instances = [];

        public function __construct(public string $base)
        {
            $this->instances['db'] = new Database($base);
        }

        public function make(string $id): object
        {
            return $id === \Illuminate\Contracts\Http\Kernel::class ? new Kernel($this) : $this->instances[$id];
        }

        public function instance(string $id, $value): void
        {
            $this->instances[$id] = $value;
        }

        public function offsetExists(mixed $offset): bool
        {
            return isset($this->instances[$offset]);
        }

        public function offsetGet(mixed $offset): mixed
        {
            return $this->instances[$offset];
        }

        public function offsetSet(mixed $offset, mixed $value): void
        {
            $this->instances[$offset] = $value;
        }

        public function offsetUnset(mixed $offset): void
        {
            unset($this->instances[$offset]);
        }
    }
}

namespace {
    function config(string $key): mixed
    {
        return null;
    }
}
