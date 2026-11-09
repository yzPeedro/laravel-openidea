# Unknown OpenAPI refs

Reports `ref: '#/components/…'` values in swagger-php attributes that point at a component nobody declares. A broken `$ref` makes the generated OpenAPI document invalid; this finds it while you write the annotation instead of when `l5-swagger:generate` or a client generator fails.

## How to use it

The check runs by itself in PHP files that use swagger-php's PHP 8 attributes or objects, on the `ref:` named argument:

```php
use OpenApi\Attributes as OA;

#[OA\Schema(schema: 'UserSchema', properties: [new OA\Property(property: 'id', type: 'integer')])]
class UserSchema {}

#[OA\Get(
    path: '/users/{user}',
    responses: [
        new OA\Response(response: 200, content: new OA\JsonContent(ref: '#/components/schemas/UserSchema')),   // fine
        new OA\Response(response: 404, content: new OA\JsonContent(ref: '#/components/schemas/NotFound')),     // reported
    ],
)]
public function show() {}
```

A call or attribute counts as swagger-php when its namespace is `OA` (the usual `use OpenApi\Attributes as OA;` alias) or ends with `OpenApi\Attributes` or `OpenApi\Annotations`.

## What you see

- **Severity:** Warning.
- **Code:** `laravel.unknown-openapi-ref`.
- **Message:** `OpenAPI component '<ref>' not found`, with the whole reference, for example `OpenAPI component '#/components/schemas/NotFound' not found`.
- **Range:** the whole string inside the quotes.

There is no quick fix. Completion inside `ref: '…'` lists the components that exist, and Ctrl+click opens their declaration (see [Packages](Packages.md)).

## Details

- **Where the components come from.** Every PHP file under `app/` (up to 20,000), `docs/` (up to 2,000) and `routes/` (up to 500) that mentions `OA\` or `OpenApi\`. These declarations become components:

| Declaration | Component |
| --- | --- |
| `Schema(schema: 'X')` | `schemas/X` |
| `Response(response: 'X')` | `responses/X` (numeric codes such as `200` and `default` are not components) |
| `Parameter(parameter: 'X')` | `parameters/X` |
| `RequestBody(request: 'X')` | `requestBodies/X` |
| `Header(header: 'X')` | `headers/X` |
| `SecurityScheme(securityScheme: 'X')` | `securitySchemes/X` |
| `Examples(example: 'X')` | `examples/X` |

- **The name must be explicit.** A schema declared without `schema:` takes its class name in swagger-php, but the plugin only indexes names written in the attribute, so a `$ref` to such a schema is reported. Add `schema: 'ClassName'` to make it known.
- **Only local component refs.** A value that doesn't start with `#/components/` (an external file, `#/paths/…`) is not checked. Neither is a string still being typed.
- **Only attributes and objects.** Docblock annotations (`@OA\Get(...)` in `/** */`) are comments to the plugin and are not read, neither for components nor for refs.
- **Refresh.** The component index is rebuilt when a file under `app/`, `docs/`, `routes/` or `config/` is saved or changes on disk.

## Settings and permissions

| Setting | Label | Default |
| --- | --- | --- |
| `unknownOpenApiRefInspection` | Report unknown OpenAPI $ref components | on |

See [Settings](Settings.md). No optional permission is needed.

## Related

- [Inspections overview](Inspections.md)
- [Packages](Packages.md): OpenAPI `$ref` completion and navigation, tags, security schemes and path parameters

---
[← Wiki home](Home.md)
