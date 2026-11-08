use openidea_plugin::Range;
use openidea_plugin::providers::{CompletionItem, CompletionKind};

pub type Method = (&'static str, &'static str);

pub const COLLECTION: &[Method] = &[
    ("all", "(): array<int, {T}>"),
    (
        "avg",
        "(string|callable|null $callback = null): float|int|null",
    ),
    ("chunk", "(int $size): Collection"),
    ("contains", "($key, $operator = null, $value = null): bool"),
    ("count", "(): int"),
    ("diff", "($items): Collection<int, {T}>"),
    ("each", "(callable $callback): Collection<int, {T}>"),
    ("every", "($key, $operator = null, $value = null): bool"),
    ("except", "($keys): Collection<int, {T}>"),
    (
        "filter",
        "(?callable $callback = null): Collection<int, {T}>",
    ),
    ("find", "($key, $default = null): ?{T}"),
    (
        "first",
        "(?callable $callback = null, $default = null): ?{T}",
    ),
    (
        "firstWhere",
        "(string $key, $operator = null, $value = null): ?{T}",
    ),
    ("flatMap", "(callable $callback): Collection"),
    ("fresh", "($with = []): Collection<int, {T}>"),
    (
        "groupBy",
        "($groupBy, bool $preserveKeys = false): Collection",
    ),
    ("implode", "($value, ?string $glue = null): string"),
    ("isEmpty", "(): bool"),
    ("isNotEmpty", "(): bool"),
    ("keyBy", "($keyBy): Collection<array-key, {T}>"),
    (
        "last",
        "(?callable $callback = null, $default = null): ?{T}",
    ),
    ("load", "($relations): Collection<int, {T}>"),
    ("loadCount", "($relations): Collection<int, {T}>"),
    ("loadMissing", "($relations): Collection<int, {T}>"),
    ("map", "(callable $callback): Collection"),
    ("mapWithKeys", "(callable $callback): Collection"),
    ("max", "($callback = null): mixed"),
    ("min", "($callback = null): mixed"),
    ("modelKeys", "(): array"),
    ("only", "($keys): Collection<int, {T}>"),
    (
        "partition",
        "($key, $operator = null, $value = null): Collection",
    ),
    ("pluck", "($value, ?string $key = null): Collection"),
    ("push", "(...$values): Collection<int, {T}>"),
    ("reduce", "(callable $callback, $initial = null): mixed"),
    ("reject", "($callback = true): Collection<int, {T}>"),
    (
        "sole",
        "($key = null, $operator = null, $value = null): {T}",
    ),
    (
        "sortBy",
        "($callback, int $options = SORT_REGULAR, bool $descending = false): Collection<int, {T}>",
    ),
    (
        "sortByDesc",
        "($callback, int $options = SORT_REGULAR): Collection<int, {T}>",
    ),
    ("sum", "($callback = null): mixed"),
    ("take", "(int $limit): Collection<int, {T}>"),
    ("toArray", "(): array"),
    ("toJson", "(int $options = 0): string"),
    ("toQuery", "(): Builder<{T}>"),
    (
        "unique",
        "($key = null, bool $strict = false): Collection<int, {T}>",
    ),
    ("values", "(): Collection<int, {T}>"),
    (
        "where",
        "($key, $operator = null, $value = null): Collection<int, {T}>",
    ),
    (
        "whereIn",
        "($key, $values, bool $strict = false): Collection<int, {T}>",
    ),
    (
        "whereNotNull",
        "(?string $key = null): Collection<int, {T}>",
    ),
];

pub const QUERY: &[Method] = &[
    ("avg", "(string $column): mixed"),
    ("chunk", "(int $count, callable $callback): bool"),
    ("count", "(string $columns = '*'): int"),
    ("cursor", "(): LazyCollection<int, {T}>"),
    (
        "decrement",
        "(string $column, float|int $amount = 1, array $extra = []): int",
    ),
    ("delete", "(): mixed"),
    ("doesntExist", "(): bool"),
    ("each", "(callable $callback, int $count = 1000): bool"),
    ("exists", "(): bool"),
    ("find", "($id, $columns = ['*']): ?{T}"),
    ("findOrFail", "($id, $columns = ['*']): {T}"),
    ("first", "($columns = ['*']): ?{T}"),
    ("firstOrFail", "($columns = ['*']): {T}"),
    ("get", "($columns = ['*']): Collection<int, {T}>"),
    (
        "increment",
        "(string $column, float|int $amount = 1, array $extra = []): int",
    ),
    ("latest", "(string $column = 'created_at'): static"),
    ("lazy", "(int $chunkSize = 1000): LazyCollection<int, {T}>"),
    ("limit", "(int $value): static"),
    ("max", "(string $column): mixed"),
    ("min", "(string $column): mixed"),
    ("oldest", "(string $column = 'created_at'): static"),
    ("orderBy", "($column, string $direction = 'asc'): static"),
    ("orderByDesc", "($column): static"),
    (
        "paginate",
        "($perPage = null, $columns = ['*'], $pageName = 'page', $page = null): LengthAwarePaginator",
    ),
    ("pluck", "(string $column, ?string $key = null): Collection"),
    (
        "simplePaginate",
        "($perPage = null, $columns = ['*'], $pageName = 'page', $page = null): Paginator",
    ),
    ("sum", "(string $column): mixed"),
    ("take", "(int $value): static"),
    ("toSql", "(): string"),
    ("update", "(array $values): int"),
    (
        "where",
        "($column, $operator = null, $value = null, string $boolean = 'and'): static",
    ),
    (
        "whereIn",
        "(string $column, $values, string $boolean = 'and', bool $not = false): static",
    ),
    (
        "whereNotNull",
        "(string|array $columns, string $boolean = 'and'): static",
    ),
    (
        "whereNull",
        "(string|array $columns, string $boolean = 'and', bool $not = false): static",
    ),
    ("with", "($relations, $callback = null): static"),
    ("withCount", "($relations): static"),
];

const ONE_OR_MANY: &[Method] = &[
    ("create", "(array $attributes = []): {T}"),
    ("createMany", "(iterable $records): Collection<int, {T}>"),
    ("createQuietly", "(array $attributes = []): {T}"),
    (
        "firstOrCreate",
        "(array $attributes = [], array $values = []): {T}",
    ),
    (
        "firstOrNew",
        "(array $attributes = [], array $values = []): {T}",
    ),
    ("forceCreate", "(array $attributes = []): {T}"),
    ("make", "(array $attributes = []): {T}"),
    ("makeMany", "(iterable $records): Collection<int, {T}>"),
    ("save", "(Model $model): {T}|false"),
    ("saveMany", "(iterable $models): iterable"),
    ("saveQuietly", "(Model $model): {T}|false"),
    (
        "updateOrCreate",
        "(array $attributes, array $values = []): {T}",
    ),
];

const ONE_OF_MANY: &[Method] = &[
    (
        "latestOfMany",
        "($column = 'id', ?string $relation = null): static",
    ),
    (
        "ofMany",
        "($column = 'id', $aggregate = 'MAX', ?string $relation = null): static",
    ),
    (
        "oldestOfMany",
        "($column = 'id', ?string $relation = null): static",
    ),
];

const BELONGS_TO: &[Method] = &[
    ("associate", "($model): Model"),
    ("dissociate", "(): Model"),
    ("getResults", "(): ?{T}"),
];

const BELONGS_TO_MANY: &[Method] = &[
    ("allRelatedIds", "(): Collection"),
    ("as", "(string $accessor): static"),
    (
        "attach",
        "($ids, array $attributes = [], bool $touch = true): void",
    ),
    (
        "create",
        "(array $attributes = [], array $joining = [], bool $touch = true): {T}",
    ),
    (
        "createMany",
        "(iterable $records, array $joinings = []): array",
    ),
    ("detach", "($ids = null, bool $touch = true): int"),
    ("findOrNew", "($id, $columns = ['*']): {T}"),
    (
        "firstOrCreate",
        "(array $attributes = [], array $values = [], array $joining = [], bool $touch = true): {T}",
    ),
    (
        "save",
        "(Model $model, array $pivotAttributes = [], bool $touch = true): {T}",
    ),
    (
        "saveMany",
        "(iterable $models, array $pivotAttributes = []): iterable",
    ),
    ("sync", "($ids, bool $detaching = true): array"),
    (
        "syncWithPivotValues",
        "($ids, array $values, bool $detaching = true): array",
    ),
    ("syncWithoutDetaching", "($ids): array"),
    ("toggle", "($ids, bool $touch = true): array"),
    (
        "updateExistingPivot",
        "($id, array $attributes, bool $touch = true): int",
    ),
    (
        "updateOrCreate",
        "(array $attributes, array $values = [], array $joining = [], bool $touch = true): {T}",
    ),
    (
        "wherePivot",
        "($column, $operator = null, $value = null, string $boolean = 'and'): static",
    ),
    (
        "wherePivotIn",
        "(string $column, $values, string $boolean = 'and', bool $not = false): static",
    ),
    (
        "wherePivotNull",
        "(string $column, string $boolean = 'and', bool $not = false): static",
    ),
    ("withPivot", "($columns): static"),
    (
        "withTimestamps",
        "(?string $createdAt = null, ?string $updatedAt = null): static",
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Model,
    Many,
    Query,
}

const QUERY_ELEMENT: &[&str] = &[
    "first",
    "firstOrFail",
    "firstOrNew",
    "firstOrCreate",
    "firstWhere",
    "sole",
    "find",
    "findOrFail",
    "findOrNew",
    "create",
    "createQuietly",
    "forceCreate",
    "make",
    "updateOrCreate",
    "getResults",
];

const QUERY_MANY: &[&str] = &["get", "all", "findMany"];

const QUERY_CHAIN: &[&str] = &[
    "query",
    "newQuery",
    "latest",
    "oldest",
    "limit",
    "take",
    "skip",
    "offset",
    "select",
    "addSelect",
    "distinct",
    "groupBy",
    "having",
    "join",
    "leftJoin",
    "when",
    "unless",
    "tap",
    "has",
    "doesntHave",
    "withTrashed",
    "onlyTrashed",
    "withoutTrashed",
    "lockForUpdate",
    "sharedLock",
    "inRandomOrder",
    "reorder",
    "without",
    "withoutGlobalScope",
    "withoutGlobalScopes",
    "forPage",
    "toBase",
];

const MANY_ELEMENT: &[&str] = &[
    "first",
    "last",
    "sole",
    "find",
    "firstWhere",
    "pop",
    "shift",
    "random",
];

const MANY_CHAIN: &[&str] = &[
    "filter",
    "reject",
    "where",
    "whereIn",
    "whereNotIn",
    "whereNull",
    "whereNotNull",
    "sortBy",
    "sortByDesc",
    "values",
    "unique",
    "take",
    "skip",
    "slice",
    "each",
    "load",
    "loadMissing",
    "loadCount",
    "fresh",
    "except",
    "only",
    "diff",
    "merge",
    "concat",
    "push",
    "reverse",
    "shuffle",
    "when",
    "unless",
    "tap",
    "makeHidden",
    "makeVisible",
];

const MODEL_SELF: &[&str] = &[
    "fresh",
    "refresh",
    "load",
    "loadMissing",
    "loadCount",
    "replicate",
    "makeHidden",
    "makeVisible",
    "setRelation",
    "unsetRelation",
];

fn chains_query(call: &str) -> bool {
    QUERY_CHAIN.contains(&call)
        || [
            "where",
            "orWhere",
            "orderBy",
            "with",
            "whereHas",
            "orWhereHas",
            "whereDoesntHave",
            "having",
            "whereRelation",
        ]
        .iter()
        .any(|p| call.starts_with(p))
}

pub fn after_call(shape: Shape, call: &str) -> Option<Shape> {
    match shape {
        Shape::Many if call == "toQuery" => Some(Shape::Query),
        Shape::Many if MANY_ELEMENT.contains(&call) => Some(Shape::Model),
        Shape::Many if MANY_CHAIN.contains(&call) => Some(Shape::Many),
        Shape::Query if QUERY_ELEMENT.contains(&call) => Some(Shape::Model),
        Shape::Query if QUERY_MANY.contains(&call) => Some(Shape::Many),
        Shape::Query if chains_query(call) => Some(Shape::Query),
        Shape::Model if MODEL_SELF.contains(&call) => Some(Shape::Model),
        Shape::Model if chains_query(call) => Some(Shape::Query),
        _ => None,
    }
}

pub fn relation_methods(kind: &str) -> Vec<Method> {
    let own: &[&[Method]] = match kind {
        "hasMany" | "morphMany" => &[ONE_OR_MANY],
        "hasOne" | "morphOne" => &[ONE_OR_MANY, ONE_OF_MANY],
        "belongsTo" | "morphTo" => &[BELONGS_TO],
        "belongsToMany" | "morphToMany" | "morphedByMany" => &[BELONGS_TO_MANY],
        _ => &[],
    };
    let mut out: Vec<Method> = own.iter().flat_map(|m| m.iter().copied()).collect();
    for m in QUERY {
        if !out.iter().any(|(name, _)| name == &m.0) {
            out.push(*m);
        }
    }
    out.sort_by_key(|(name, _)| *name);
    out
}

pub fn method_items<N: AsRef<str>>(
    methods: &[(N, &str)],
    model: &str,
    typed: &str,
    replace: Range,
) -> Vec<CompletionItem> {
    let lower = typed.to_lowercase();
    methods
        .iter()
        .filter(|(name, _)| name.as_ref().to_lowercase().contains(&lower))
        .map(|(name, signature)| {
            let name = name.as_ref();
            let rank = if name.to_lowercase().starts_with(&lower) {
                '0'
            } else {
                '1'
            };
            CompletionItem {
                detail: Some(signature.replace("{T}", model)),
                insert_text: format!("{name}($0)"),
                snippet: true,
                replace: Some(replace),
                sort_text: Some(format!("{rank}{name}")),
                ..CompletionItem::new(name, CompletionKind::Method)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_kinds_get_their_own_methods_and_the_query_ones() {
        let names = |kind: &str| {
            relation_methods(kind)
                .into_iter()
                .map(|(n, _)| n)
                .collect::<Vec<_>>()
        };
        let many = names("hasMany");
        assert!(
            many.contains(&"createMany") && many.contains(&"where") && !many.contains(&"attach")
        );
        let pivot = names("belongsToMany");
        assert!(pivot.contains(&"sync") && pivot.contains(&"attach") && pivot.contains(&"get"));
        assert!(names("belongsTo").contains(&"associate"));
        assert!(names("hasOne").contains(&"latestOfMany"));
        assert_eq!(many.iter().filter(|n| **n == "create").count(), 1);
    }
}
