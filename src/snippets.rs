use crate::php::{Kind, tokenize};
use openidea_plugin::providers::{CompletionItem, CompletionKind};
use openidea_plugin::{Range, TextEdit};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    File,
    Class,
    Code,
}

pub struct Snippet {
    pub prefix: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    pub places: &'static [Place],
    pub imports: &'static [&'static str],
    pub body: &'static str,
}

const FILE: &[Place] = &[Place::File];
const CLASS: &[Place] = &[Place::Class];
const CODE: &[Place] = &[Place::File, Place::Code];

const fn s(
    prefix: &'static str,
    group: &'static str,
    description: &'static str,
    places: &'static [Place],
    body: &'static str,
) -> Snippet {
    Snippet {
        prefix,
        group,
        description,
        places,
        imports: &[],
        body,
    }
}

const fn u(
    prefix: &'static str,
    group: &'static str,
    description: &'static str,
    places: &'static [Place],
    imports: &'static [&'static str],
    body: &'static str,
) -> Snippet {
    Snippet {
        prefix,
        group,
        description,
        places,
        imports,
        body,
    }
}

const PHP: &str = "PHP";
const MEMBERS: &str = "Class members";
const FLOW: &str = "Control flow";
const MODEL: &str = "Eloquent models";
const RELATION: &str = "Eloquent relations";
const QUERY: &str = "Eloquent and queries";
const CLASSES: &str = "Laravel classes";
const ROUTES: &str = "Routes";
const MIGRATION: &str = "Migrations";
const APP: &str = "Laravel helpers";
const TESTS: &str = "Tests (PHPUnit)";
const PEST: &str = "Pest";

const BUILDER: &str = "Illuminate\\Database\\Eloquent\\Builder";
const ATTRIBUTE: &str = "Illuminate\\Database\\Eloquent\\Casts\\Attribute";
const REQUEST: &str = "Illuminate\\Http\\Request";

pub const SNIPPETS: &[Snippet] = &[
    s("php", PHP, "PHP opening tag", FILE, "<?php\n\n$0"),
    s(
        "strict",
        PHP,
        "declare(strict_types=1);",
        FILE,
        "declare(strict_types=1);\n$0",
    ),
    s(
        "class",
        PHP,
        "Class named after the file",
        FILE,
        "class ${1:${TM_FILENAME_BASE}}\n{\n\t$0\n}",
    ),
    s(
        "fclass",
        PHP,
        "Final class",
        FILE,
        "final class ${1:${TM_FILENAME_BASE}}\n{\n\t$0\n}",
    ),
    s(
        "aclass",
        PHP,
        "Abstract class",
        FILE,
        "abstract class ${1:${TM_FILENAME_BASE}}\n{\n\t$0\n}",
    ),
    s(
        "rclass",
        PHP,
        "Readonly class",
        FILE,
        "readonly class ${1:${TM_FILENAME_BASE}}\n{\n\tpublic function __construct(\n\t\tpublic ${2:string} \\$${3:name},$0\n\t) {}\n}",
    ),
    s(
        "cext",
        PHP,
        "Class that extends another",
        FILE,
        "class ${1:${TM_FILENAME_BASE}} extends ${2:Parent}\n{\n\t$0\n}",
    ),
    s(
        "interface",
        PHP,
        "Interface",
        FILE,
        "interface ${1:${TM_FILENAME_BASE}}\n{\n\tpublic function ${2:name}(${3}): ${4:void};$0\n}",
    ),
    s(
        "trait",
        PHP,
        "Trait",
        FILE,
        "trait ${1:${TM_FILENAME_BASE}}\n{\n\t$0\n}",
    ),
    s(
        "enum",
        PHP,
        "Pure enum",
        FILE,
        "enum ${1:${TM_FILENAME_BASE}}\n{\n\tcase ${2:First};$0\n}",
    ),
    s(
        "enums",
        PHP,
        "String-backed enum",
        FILE,
        "enum ${1:${TM_FILENAME_BASE}}: string\n{\n\tcase ${2:Draft} = '${3:draft}';$0\n}",
    ),
    s(
        "enumi",
        PHP,
        "Int-backed enum",
        FILE,
        "enum ${1:${TM_FILENAME_BASE}}: int\n{\n\tcase ${2:Low} = ${3:1};$0\n}",
    ),
    s(
        "func",
        PHP,
        "Function declaration",
        FILE,
        "function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
    ),
    s(
        "pubf",
        MEMBERS,
        "public function",
        CLASS,
        "public function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
    ),
    s(
        "prof",
        MEMBERS,
        "protected function",
        CLASS,
        "protected function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
    ),
    s(
        "prif",
        MEMBERS,
        "private function",
        CLASS,
        "private function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
    ),
    s(
        "pubsf",
        MEMBERS,
        "public static function",
        CLASS,
        "public static function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
    ),
    s(
        "prosf",
        MEMBERS,
        "protected static function",
        CLASS,
        "protected static function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
    ),
    s(
        "prisf",
        MEMBERS,
        "private static function",
        CLASS,
        "private static function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
    ),
    s(
        "absf",
        MEMBERS,
        "abstract function",
        CLASS,
        "abstract ${1:public} function ${2:name}(${3}): ${4:void};$0",
    ),
    s(
        "ctor",
        MEMBERS,
        "Constructor",
        CLASS,
        "public function __construct(${1})\n{\n\t$0\n}",
    ),
    s(
        "ctorp",
        MEMBERS,
        "Constructor with promoted properties",
        CLASS,
        "public function __construct(\n\tprivate readonly ${1:Type} \\$${2:name},$0\n) {}",
    ),
    s(
        "pubp",
        MEMBERS,
        "public property",
        CLASS,
        "public ${1:string} \\$${2:name};$0",
    ),
    s(
        "prop",
        MEMBERS,
        "protected property",
        CLASS,
        "protected ${1:string} \\$${2:name};$0",
    ),
    s(
        "prip",
        MEMBERS,
        "private property",
        CLASS,
        "private ${1:string} \\$${2:name};$0",
    ),
    s(
        "pubro",
        MEMBERS,
        "public readonly property",
        CLASS,
        "public readonly ${1:string} \\$${2:name};$0",
    ),
    s(
        "const",
        MEMBERS,
        "Class constant",
        CLASS,
        "${1:public} const ${2:NAME} = ${3:'value'};$0",
    ),
    s(
        "case",
        MEMBERS,
        "Enum case with a value",
        CLASS,
        "case ${1:Name} = '${2:value}';$0",
    ),
    s(
        "get",
        MEMBERS,
        "Getter",
        CLASS,
        "public function get${1:Name}(): ${2:mixed}\n{\n\treturn \\$this->${3:name};\n}$0",
    ),
    s(
        "set",
        MEMBERS,
        "Fluent setter",
        CLASS,
        "public function set${1:Name}(${2:mixed} \\$${3:value}): static\n{\n\t\\$this->${4:name} = \\$${3:value};\n\n\treturn \\$this;\n}$0",
    ),
    s(
        "invoke",
        MEMBERS,
        "__invoke()",
        CLASS,
        "public function __invoke(${1}): ${2:mixed}\n{\n\t$0\n}",
    ),
    s(
        "tostring",
        MEMBERS,
        "__toString()",
        CLASS,
        "public function __toString(): string\n{\n\treturn $0;\n}",
    ),
    s(
        "magicget",
        MEMBERS,
        "__get()",
        CLASS,
        "public function __get(string \\$name): mixed\n{\n\t$0\n}",
    ),
    s(
        "magicset",
        MEMBERS,
        "__set()",
        CLASS,
        "public function __set(string \\$name, mixed \\$value): void\n{\n\t$0\n}",
    ),
    s(
        "magiccall",
        MEMBERS,
        "__call()",
        CLASS,
        "public function __call(string \\$method, array \\$arguments): mixed\n{\n\t$0\n}",
    ),
    s(
        "magiccallstatic",
        MEMBERS,
        "__callStatic()",
        CLASS,
        "public static function __callStatic(string \\$method, array \\$arguments): mixed\n{\n\t$0\n}",
    ),
    s("if", FLOW, "if", CODE, "if (${1:\\$condition}) {\n\t$0\n}"),
    s(
        "ifel",
        FLOW,
        "if … else",
        CODE,
        "if (${1:\\$condition}) {\n\t$2\n} else {\n\t$0\n}",
    ),
    s(
        "elif",
        FLOW,
        "elseif",
        CODE,
        "elseif (${1:\\$condition}) {\n\t$0\n}",
    ),
    s("else", FLOW, "else", CODE, "else {\n\t$0\n}"),
    s(
        "foreach",
        FLOW,
        "foreach",
        CODE,
        "foreach (${1:\\$items} as ${2:\\$item}) {\n\t$0\n}",
    ),
    s(
        "foreachk",
        FLOW,
        "foreach with key",
        CODE,
        "foreach (${1:\\$items} as ${2:\\$key} => ${3:\\$value}) {\n\t$0\n}",
    ),
    s(
        "for",
        FLOW,
        "for",
        CODE,
        "for (\\$${1:i} = 0; \\$${1:i} < ${2:\\$count}; \\$${1:i}++) {\n\t$0\n}",
    ),
    s(
        "while",
        FLOW,
        "while",
        CODE,
        "while (${1:\\$condition}) {\n\t$0\n}",
    ),
    s(
        "do",
        FLOW,
        "do … while",
        CODE,
        "do {\n\t$0\n} while (${1:\\$condition});",
    ),
    s(
        "switch",
        FLOW,
        "switch",
        CODE,
        "switch (${1:\\$value}) {\n\tcase ${2:'a'}:\n\t\t$0\n\t\tbreak;\n\tdefault:\n\t\tbreak;\n}",
    ),
    s(
        "match",
        FLOW,
        "match expression",
        CODE,
        "match (${1:\\$value}) {\n\t${2:'a'} => ${3:1},\n\tdefault => ${4:null},\n};$0",
    ),
    s(
        "try",
        FLOW,
        "try … catch",
        CODE,
        "try {\n\t$1\n} catch (${2:\\Throwable} \\$${3:e}) {\n\t$0\n}",
    ),
    s(
        "tryf",
        FLOW,
        "try … finally",
        CODE,
        "try {\n\t$1\n} finally {\n\t$0\n}",
    ),
    s(
        "trycf",
        FLOW,
        "try … catch … finally",
        CODE,
        "try {\n\t$1\n} catch (${2:\\Throwable} \\$${3:e}) {\n\t$4\n} finally {\n\t$0\n}",
    ),
    s(
        "throw",
        FLOW,
        "throw new exception",
        CODE,
        "throw new ${1:\\RuntimeException}('${2:message}');$0",
    ),
    s(
        "fn",
        FLOW,
        "Arrow function",
        CODE,
        "fn (${1:\\$item}) => ${0:\\$item}",
    ),
    s(
        "closure",
        FLOW,
        "Closure with use",
        CODE,
        "function (${1}) use (${2}) {\n\t$0\n}",
    ),
    s("ret", FLOW, "return", CODE, "return ${0};"),
    s("dd", APP, "dd()", CODE, "dd(${0});"),
    s("dump", APP, "dump()", CODE, "dump(${0});"),
    s(
        "fillable",
        MODEL,
        "$fillable",
        CLASS,
        "protected \\$fillable = [\n\t'${1:name}',$0\n];",
    ),
    s(
        "guarded",
        MODEL,
        "$guarded",
        CLASS,
        "protected \\$guarded = [${1:'id'}];$0",
    ),
    s(
        "hidden",
        MODEL,
        "$hidden",
        CLASS,
        "protected \\$hidden = [\n\t'${1:password}',$0\n];",
    ),
    s(
        "visible",
        MODEL,
        "$visible",
        CLASS,
        "protected \\$visible = [\n\t'${1:name}',$0\n];",
    ),
    s(
        "appends",
        MODEL,
        "$appends",
        CLASS,
        "protected \\$appends = [\n\t'${1:full_name}',$0\n];",
    ),
    s(
        "with",
        MODEL,
        "$with (always eager load)",
        CLASS,
        "protected \\$with = ['${1:relation}'];$0",
    ),
    s(
        "table",
        MODEL,
        "$table",
        CLASS,
        "protected \\$table = '${1:table}';$0",
    ),
    s(
        "primarykey",
        MODEL,
        "$primaryKey and key type",
        CLASS,
        "protected \\$primaryKey = '${1:uuid}';\n\nprotected \\$keyType = '${2:string}';\n\npublic \\$incrementing = false;$0",
    ),
    u(
        "hasfactory",
        MODEL,
        "use HasFactory",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Factories\\HasFactory"],
        "use HasFactory;$0",
    ),
    s(
        "timestamps",
        MODEL,
        "Turn timestamps off",
        CLASS,
        "public \\$timestamps = false;$0",
    ),
    s(
        "connection",
        MODEL,
        "$connection",
        CLASS,
        "protected \\$connection = '${1:mysql}';$0",
    ),
    s(
        "perpage",
        MODEL,
        "$perPage",
        CLASS,
        "protected \\$perPage = ${1:25};$0",
    ),
    s(
        "touches",
        MODEL,
        "$touches",
        CLASS,
        "protected \\$touches = ['${1:post}'];$0",
    ),
    s(
        "casts",
        MODEL,
        "casts() method",
        CLASS,
        "protected function casts(): array\n{\n\treturn [\n\t\t'${1:published_at}' => '${2:datetime}',$0\n\t];\n}",
    ),
    s(
        "castsp",
        MODEL,
        "$casts property (Laravel 10 and older)",
        CLASS,
        "protected \\$casts = [\n\t'${1:published_at}' => '${2:datetime}',$0\n];",
    ),
    u(
        "scope",
        MODEL,
        "Local scope",
        CLASS,
        &[BUILDER],
        "public function scope${1:Active}(Builder \\$query): void\n{\n\t\\$query->${2:where('active', true)};$0\n}",
    ),
    u(
        "scopeattr",
        MODEL,
        "Local scope with #[Scope] (Laravel 12)",
        CLASS,
        &[BUILDER, "Illuminate\\Database\\Eloquent\\Attributes\\Scope"],
        "#[Scope]\nprotected function ${1:active}(Builder \\$query): void\n{\n\t\\$query->${2:where('active', true)};$0\n}",
    ),
    u(
        "accessor",
        MODEL,
        "Accessor (Attribute::make get)",
        CLASS,
        &[ATTRIBUTE],
        "protected function ${1:name}(): Attribute\n{\n\treturn Attribute::make(\n\t\tget: fn (${2:string} \\$value) => ${3:ucfirst(\\$value)},\n\t);\n}$0",
    ),
    u(
        "mutator",
        MODEL,
        "Mutator (Attribute::make set)",
        CLASS,
        &[ATTRIBUTE],
        "protected function ${1:name}(): Attribute\n{\n\treturn Attribute::make(\n\t\tset: fn (${2:string} \\$value) => ${3:strtolower(\\$value)},\n\t);\n}$0",
    ),
    u(
        "attr",
        MODEL,
        "Accessor and mutator",
        CLASS,
        &[ATTRIBUTE],
        "protected function ${1:name}(): Attribute\n{\n\treturn Attribute::make(\n\t\tget: fn (${2:string} \\$value) => ${3:\\$value},\n\t\tset: fn (${2:string} \\$value) => ${4:\\$value},\n\t);\n}$0",
    ),
    s(
        "booted",
        MODEL,
        "booted() with a model event",
        CLASS,
        "protected static function booted(): void\n{\n\tstatic::${1:creating}(function (${2:self} \\$model) {\n\t\t$0\n\t});\n}",
    ),
    u(
        "hasone",
        RELATION,
        "hasOne relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\HasOne"],
        "public function ${1:profile}(): HasOne\n{\n\treturn \\$this->hasOne(${2:Profile}::class);\n}$0",
    ),
    u(
        "hasmany",
        RELATION,
        "hasMany relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\HasMany"],
        "public function ${1:posts}(): HasMany\n{\n\treturn \\$this->hasMany(${2:Post}::class);\n}$0",
    ),
    u(
        "belongsto",
        RELATION,
        "belongsTo relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\BelongsTo"],
        "public function ${1:user}(): BelongsTo\n{\n\treturn \\$this->belongsTo(${2:User}::class);\n}$0",
    ),
    u(
        "belongstomany",
        RELATION,
        "belongsToMany relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\BelongsToMany"],
        "public function ${1:roles}(): BelongsToMany\n{\n\treturn \\$this->belongsToMany(${2:Role}::class)${3:->withTimestamps()};\n}$0",
    ),
    u(
        "hasonethrough",
        RELATION,
        "hasOneThrough relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\HasOneThrough"],
        "public function ${1:owner}(): HasOneThrough\n{\n\treturn \\$this->hasOneThrough(${2:Owner}::class, ${3:Car}::class);\n}$0",
    ),
    u(
        "hasmanythrough",
        RELATION,
        "hasManyThrough relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\HasManyThrough"],
        "public function ${1:deployments}(): HasManyThrough\n{\n\treturn \\$this->hasManyThrough(${2:Deployment}::class, ${3:Environment}::class);\n}$0",
    ),
    u(
        "morphto",
        RELATION,
        "morphTo relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\MorphTo"],
        "public function ${1:commentable}(): MorphTo\n{\n\treturn \\$this->morphTo();\n}$0",
    ),
    u(
        "morphone",
        RELATION,
        "morphOne relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\MorphOne"],
        "public function ${1:image}(): MorphOne\n{\n\treturn \\$this->morphOne(${2:Image}::class, '${3:imageable}');\n}$0",
    ),
    u(
        "morphmany",
        RELATION,
        "morphMany relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\MorphMany"],
        "public function ${1:comments}(): MorphMany\n{\n\treturn \\$this->morphMany(${2:Comment}::class, '${3:commentable}');\n}$0",
    ),
    u(
        "morphtomany",
        RELATION,
        "morphToMany relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\MorphToMany"],
        "public function ${1:tags}(): MorphToMany\n{\n\treturn \\$this->morphToMany(${2:Tag}::class, '${3:taggable}');\n}$0",
    ),
    u(
        "morphedbymany",
        RELATION,
        "morphedByMany relation",
        CLASS,
        &["Illuminate\\Database\\Eloquent\\Relations\\MorphToMany"],
        "public function ${1:posts}(): MorphToMany\n{\n\treturn \\$this->morphedByMany(${2:Post}::class, '${3:taggable}');\n}$0",
    ),
    s(
        "qwhere",
        QUERY,
        "Query with where()",
        CODE,
        "${1:Model}::query()\n\t->where('${2:column}', ${3:\\$value})\n\t->${4:get}();$0",
    ),
    s(
        "qfirst",
        QUERY,
        "First matching model or fail",
        CODE,
        "${1:Model}::where('${2:column}', ${3:\\$value})->firstOrFail();$0",
    ),
    s(
        "qfind",
        QUERY,
        "findOrFail()",
        CODE,
        "\\$${1:model} = ${2:Model}::findOrFail(${3:\\$id});$0",
    ),
    s(
        "qcreate",
        QUERY,
        "create()",
        CODE,
        "${1:Model}::create([\n\t'${2:name}' => ${3:\\$name},$0\n]);",
    ),
    s(
        "qupdate",
        QUERY,
        "update() on a model",
        CODE,
        "\\$${1:model}->update([\n\t'${2:name}' => ${3:\\$name},$0\n]);",
    ),
    s(
        "qfoc",
        QUERY,
        "firstOrCreate()",
        CODE,
        "${1:Model}::firstOrCreate(\n\t['${2:email}' => ${3:\\$email}],\n\t[${4}]\n);$0",
    ),
    s(
        "quoc",
        QUERY,
        "updateOrCreate()",
        CODE,
        "${1:Model}::updateOrCreate(\n\t['${2:email}' => ${3:\\$email}],\n\t[${4}]\n);$0",
    ),
    s(
        "qwith",
        QUERY,
        "Eager load with with()",
        CODE,
        "${1:Model}::with('${2:relation}')->${3:get}();$0",
    ),
    s(
        "qwherehas",
        QUERY,
        "whereHas()",
        CODE,
        "->whereHas('${1:relation}', fn (\\$query) => \\$query->where('${2:column}', ${3:\\$value}))$0",
    ),
    s(
        "qpaginate",
        QUERY,
        "Paginated query",
        CODE,
        "${1:Model}::query()\n\t->latest()\n\t->paginate(${2:15});$0",
    ),
    s(
        "qchunk",
        QUERY,
        "chunkById()",
        CODE,
        "${1:Model}::query()->chunkById(${2:200}, function (\\$${3:models}) {\n\tforeach (\\$${3:models} as \\$${4:model}) {\n\t\t$0\n\t}\n});",
    ),
    s(
        "qlazy",
        QUERY,
        "lazy() iteration",
        CODE,
        "foreach (${1:Model}::query()->lazy() as \\$${2:model}) {\n\t$0\n}",
    ),
    s(
        "dbtrans",
        QUERY,
        "DB::transaction()",
        CODE,
        "DB::transaction(function () {\n\t$0\n});",
    ),
    s(
        "dbtable",
        QUERY,
        "DB::table() query",
        CODE,
        "DB::table('${1:table}')\n\t->where('${2:column}', ${3:\\$value})\n\t->${4:get}();$0",
    ),
    s(
        "dbselect",
        QUERY,
        "DB::select() with bindings",
        CODE,
        "DB::select('${1:select * from users where id = ?}', [${2:\\$id}]);$0",
    ),
    u(
        "index",
        CLASSES,
        "Controller index()",
        CLASS,
        &[],
        "public function index()\n{\n\t$0\n}",
    ),
    u(
        "create",
        CLASSES,
        "Controller create()",
        CLASS,
        &[],
        "public function create()\n{\n\treturn view('${1:resource.create}');$0\n}",
    ),
    u(
        "store",
        CLASSES,
        "Controller store()",
        CLASS,
        &[REQUEST],
        "public function store(Request \\$request)\n{\n\t\\$validated = \\$request->validate([\n\t\t'${1:name}' => ['${2:required}'],\n\t]);\n\t$0\n}",
    ),
    u(
        "show",
        CLASSES,
        "Controller show()",
        CLASS,
        &[],
        "public function show(${1:Model} \\$${2:model})\n{\n\t$0\n}",
    ),
    u(
        "edit",
        CLASSES,
        "Controller edit()",
        CLASS,
        &[],
        "public function edit(${1:Model} \\$${2:model})\n{\n\t$0\n}",
    ),
    u(
        "update",
        CLASSES,
        "Controller update()",
        CLASS,
        &[REQUEST],
        "public function update(Request \\$request, ${1:Model} \\$${2:model})\n{\n\t$0\n}",
    ),
    u(
        "destroy",
        CLASSES,
        "Controller destroy()",
        CLASS,
        &[],
        "public function destroy(${1:Model} \\$${2:model})\n{\n\t\\$${2:model}->delete();\n\t$0\n}",
    ),
    u(
        "resource",
        CLASSES,
        "The seven resource controller methods",
        CLASS,
        &[REQUEST],
        "public function index()\n{\n\t$0\n}\n\npublic function create()\n{\n}\n\npublic function store(Request \\$request)\n{\n}\n\npublic function show(${1:Model} \\$${2:model})\n{\n}\n\npublic function edit(${1:Model} \\$${2:model})\n{\n}\n\npublic function update(Request \\$request, ${1:Model} \\$${2:model})\n{\n}\n\npublic function destroy(${1:Model} \\$${2:model})\n{\n}",
    ),
    s(
        "authorize",
        CLASSES,
        "Form Request authorize()",
        CLASS,
        "public function authorize(): bool\n{\n\treturn ${1:true};\n}$0",
    ),
    s(
        "rules",
        CLASSES,
        "Form Request rules()",
        CLASS,
        "public function rules(): array\n{\n\treturn [\n\t\t'${1:name}' => ['${2:required}', '${3:string}'],$0\n\t];\n}",
    ),
    s(
        "messages",
        CLASSES,
        "Form Request messages()",
        CLASS,
        "public function messages(): array\n{\n\treturn [\n\t\t'${1:name}.${2:required}' => '${3:The name is required.}',$0\n\t];\n}",
    ),
    s(
        "prepare",
        CLASSES,
        "prepareForValidation()",
        CLASS,
        "protected function prepareForValidation(): void\n{\n\t\\$this->merge([\n\t\t'${1:slug}' => ${2:\\$this->slug},$0\n\t]);\n}",
    ),
    s(
        "handle",
        CLASSES,
        "handle() for jobs, listeners and commands",
        CLASS,
        "public function handle(${1}): ${2:void}\n{\n\t$0\n}",
    ),
    s(
        "failed",
        CLASSES,
        "Job failed()",
        CLASS,
        "public function failed(?\\Throwable \\$exception): void\n{\n\t$0\n}",
    ),
    s(
        "tries",
        CLASSES,
        "Job $tries and $backoff",
        CLASS,
        "public \\$tries = ${1:3};\n\npublic \\$backoff = ${2:10};$0",
    ),
    s(
        "signature",
        CLASSES,
        "Artisan command signature and description",
        CLASS,
        "protected \\$signature = '${1:app:command} {${2:argument}} {--${3:option}}';\n\nprotected \\$description = '${4:Command description}';$0",
    ),
    u(
        "mhandle",
        CLASSES,
        "Middleware handle()",
        CLASS,
        &[
            REQUEST,
            "Closure",
            "Symfony\\Component\\HttpFoundation\\Response",
        ],
        "public function handle(Request \\$request, Closure \\$next): Response\n{\n\t$0\n\n\treturn \\$next(\\$request);\n}",
    ),
    u(
        "policy",
        CLASSES,
        "Policy methods",
        CLASS,
        &[],
        "public function viewAny(${1:User} \\$user): bool\n{\n\treturn ${2:true};\n}\n\npublic function view(${1:User} \\$user, ${3:Model} \\$${4:model}): bool\n{\n\treturn ${5:true};\n}\n\npublic function create(${1:User} \\$user): bool\n{\n\treturn ${6:true};\n}\n\npublic function update(${1:User} \\$user, ${3:Model} \\$${4:model}): bool\n{\n\treturn \\$user->id === \\$${4:model}->user_id;\n}\n\npublic function delete(${1:User} \\$user, ${3:Model} \\$${4:model}): bool\n{\n\treturn \\$user->id === \\$${4:model}->user_id;\n}$0",
    ),
    s(
        "register",
        CLASSES,
        "Service provider register()",
        CLASS,
        "public function register(): void\n{\n\t\\$this->app->${1:singleton}(${2:Service}::class, fn (\\$app) => new ${2:Service}());$0\n}",
    ),
    s(
        "boot",
        CLASSES,
        "Service provider boot()",
        CLASS,
        "public function boot(): void\n{\n\t$0\n}",
    ),
    s(
        "via",
        CLASSES,
        "Notification via()",
        CLASS,
        "public function via(object \\$notifiable): array\n{\n\treturn ['${1:mail}'];$0\n}",
    ),
    u(
        "tomail",
        CLASSES,
        "Notification toMail()",
        CLASS,
        &["Illuminate\\Notifications\\Messages\\MailMessage"],
        "public function toMail(object \\$notifiable): MailMessage\n{\n\treturn (new MailMessage)\n\t\t->subject('${1:Subject}')\n\t\t->line('${2:Line}')\n\t\t->action('${3:Open}', url('${4:/}'));$0\n}",
    ),
    s(
        "toarray",
        CLASSES,
        "toArray() for notifications and resources",
        CLASS,
        "public function toArray(${1:object \\$notifiable}): array\n{\n\treturn [\n\t\t'${2:id}' => \\$this->${2:id},$0\n\t];\n}",
    ),
    u(
        "envelope",
        CLASSES,
        "Mailable envelope() and content()",
        CLASS,
        &[
            "Illuminate\\Mail\\Mailables\\Envelope",
            "Illuminate\\Mail\\Mailables\\Content",
        ],
        "public function envelope(): Envelope\n{\n\treturn new Envelope(subject: '${1:Subject}');\n}\n\npublic function content(): Content\n{\n\treturn new Content(view: '${2:emails.name}');$0\n}",
    ),
    s(
        "definition",
        CLASSES,
        "Factory definition()",
        CLASS,
        "public function definition(): array\n{\n\treturn [\n\t\t'${1:name}' => fake()->${2:name}(),$0\n\t];\n}",
    ),
    s(
        "state",
        CLASSES,
        "Factory state",
        CLASS,
        "public function ${1:suspended}(): static\n{\n\treturn \\$this->state(fn (array \\$attributes) => [\n\t\t'${2:suspended_at}' => ${3:now()},$0\n\t]);\n}",
    ),
    s(
        "run",
        CLASSES,
        "Seeder run()",
        CLASS,
        "public function run(): void\n{\n\t$0\n}",
    ),
    s(
        "render",
        CLASSES,
        "render() for components",
        CLASS,
        "public function render()\n{\n\treturn view('${1:components.name}');$0\n}",
    ),
    s(
        "observer",
        CLASSES,
        "Observer events",
        CLASS,
        "public function created(${1:Model} \\$${2:model}): void\n{\n\t$0\n}\n\npublic function updated(${1:Model} \\$${2:model}): void\n{\n}\n\npublic function deleted(${1:Model} \\$${2:model}): void\n{\n}",
    ),
    s(
        "up",
        MIGRATION,
        "up() creating a table",
        CLASS,
        "public function up(): void\n{\n\tSchema::create('${1:table}', function (Blueprint \\$table) {\n\t\t\\$table->id();\n\t\t$0\n\t\t\\$table->timestamps();\n\t});\n}",
    ),
    s(
        "uptable",
        MIGRATION,
        "up() changing a table",
        CLASS,
        "public function up(): void\n{\n\tSchema::table('${1:table}', function (Blueprint \\$table) {\n\t\t$0\n\t});\n}",
    ),
    s(
        "down",
        MIGRATION,
        "down() dropping a table",
        CLASS,
        "public function down(): void\n{\n\tSchema::dropIfExists('${1:table}');$0\n}",
    ),
    u(
        "migration",
        MIGRATION,
        "Anonymous migration class",
        FILE,
        &[
            "Illuminate\\Database\\Migrations\\Migration",
            "Illuminate\\Database\\Schema\\Blueprint",
            "Illuminate\\Support\\Facades\\Schema",
        ],
        "return new class extends Migration\n{\n\tpublic function up(): void\n\t{\n\t\tSchema::create('${1:table}', function (Blueprint \\$table) {\n\t\t\t\\$table->id();\n\t\t\t$0\n\t\t\t\\$table->timestamps();\n\t\t});\n\t}\n\n\tpublic function down(): void\n\t{\n\t\tSchema::dropIfExists('${1:table}');\n\t}\n};",
    ),
    s(
        "foreign",
        MIGRATION,
        "foreignId()->constrained()",
        CODE,
        "\\$table->foreignId('${1:user_id}')->constrained()->${2:cascadeOnDelete}();$0",
    ),
    s(
        "rget",
        ROUTES,
        "Route::get()",
        CODE,
        "Route::get('${1:/path}', [${2:Controller}::class, '${3:index}'])->name('${4:name}');$0",
    ),
    s(
        "rpost",
        ROUTES,
        "Route::post()",
        CODE,
        "Route::post('${1:/path}', [${2:Controller}::class, '${3:store}'])->name('${4:name}');$0",
    ),
    s(
        "rput",
        ROUTES,
        "Route::put()",
        CODE,
        "Route::put('${1:/path}', [${2:Controller}::class, '${3:update}'])->name('${4:name}');$0",
    ),
    s(
        "rpatch",
        ROUTES,
        "Route::patch()",
        CODE,
        "Route::patch('${1:/path}', [${2:Controller}::class, '${3:update}'])->name('${4:name}');$0",
    ),
    s(
        "rdelete",
        ROUTES,
        "Route::delete()",
        CODE,
        "Route::delete('${1:/path}', [${2:Controller}::class, '${3:destroy}'])->name('${4:name}');$0",
    ),
    s(
        "rview",
        ROUTES,
        "Route::view()",
        CODE,
        "Route::view('${1:/path}', '${2:view}')->name('${3:name}');$0",
    ),
    s(
        "rredirect",
        ROUTES,
        "Route::redirect()",
        CODE,
        "Route::redirect('${1:/here}', '${2:/there}');$0",
    ),
    s(
        "rresource",
        ROUTES,
        "Route::resource()",
        CODE,
        "Route::resource('${1:photos}', ${2:PhotoController}::class);$0",
    ),
    s(
        "rapi",
        ROUTES,
        "Route::apiResource()",
        CODE,
        "Route::apiResource('${1:photos}', ${2:PhotoController}::class);$0",
    ),
    s(
        "rgroup",
        ROUTES,
        "Route group with middleware",
        CODE,
        "Route::middleware(['${1:auth}'])->group(function () {\n\t$0\n});",
    ),
    s(
        "rprefix",
        ROUTES,
        "Route group with prefix and name",
        CODE,
        "Route::prefix('${1:admin}')->name('${1:admin}.')->group(function () {\n\t$0\n});",
    ),
    s(
        "rcontroller",
        ROUTES,
        "Route::controller() group",
        CODE,
        "Route::controller(${1:Controller}::class)->group(function () {\n\tRoute::get('${2:/path}', '${3:index}');$0\n});",
    ),
    s(
        "rfallback",
        ROUTES,
        "Route::fallback()",
        CODE,
        "Route::fallback(function () {\n\t$0\n});",
    ),
    s(
        "validate",
        APP,
        "$request->validate()",
        CODE,
        "\\$validated = \\$request->validate([\n\t'${1:name}' => ['${2:required}', '${3:string}'],$0\n]);",
    ),
    s(
        "validator",
        APP,
        "Validator::make()",
        CODE,
        "\\$validator = Validator::make(\\$${1:data}, [\n\t'${2:name}' => ['${3:required}'],\n]);$0",
    ),
    s(
        "retview",
        APP,
        "return view()",
        CODE,
        "return view('${1:name}', [\n\t'${2:key}' => ${3:\\$value},\n]);$0",
    ),
    s(
        "retjson",
        APP,
        "return response()->json()",
        CODE,
        "return response()->json(${1:\\$data}, ${2:200});$0",
    ),
    s(
        "retredirect",
        APP,
        "return redirect()->route()",
        CODE,
        "return redirect()->route('${1:name}')->with('${2:status}', '${3:Saved}');$0",
    ),
    s(
        "back",
        APP,
        "return back() with a message",
        CODE,
        "return back()->with('${1:status}', '${2:Saved}');$0",
    ),
    s(
        "abortif",
        APP,
        "abort_if()",
        CODE,
        "abort_if(${1:\\$condition}, ${2:403});$0",
    ),
    s(
        "abortunless",
        APP,
        "abort_unless()",
        CODE,
        "abort_unless(${1:\\$condition}, ${2:403});$0",
    ),
    s(
        "gate",
        APP,
        "Gate::authorize()",
        CODE,
        "Gate::authorize('${1:update}', ${2:\\$model});$0",
    ),
    s(
        "authuser",
        APP,
        "The signed-in user",
        CODE,
        "\\$user = auth()->user();$0",
    ),
    s(
        "cremember",
        APP,
        "Cache::remember()",
        CODE,
        "Cache::remember('${1:key}', now()->addMinutes(${2:10}), fn () => ${3:\\$value});$0",
    ),
    s(
        "cforever",
        APP,
        "Cache::rememberForever()",
        CODE,
        "Cache::rememberForever('${1:key}', fn () => ${2:\\$value});$0",
    ),
    s(
        "cforget",
        APP,
        "Cache::forget()",
        CODE,
        "Cache::forget('${1:key}');$0",
    ),
    s(
        "clock",
        APP,
        "Cache::lock()",
        CODE,
        "Cache::lock('${1:key}', ${2:10})->get(function () {\n\t$0\n});",
    ),
    s(
        "dispatch",
        APP,
        "Dispatch a job",
        CODE,
        "${1:Job}::dispatch(${2});$0",
    ),
    s(
        "event",
        APP,
        "Fire an event",
        CODE,
        "event(new ${1:Event}(${2}));$0",
    ),
    s(
        "chain",
        APP,
        "Bus::chain()",
        CODE,
        "Bus::chain([\n\tnew ${1:FirstJob},\n\tnew ${2:SecondJob},\n])->dispatch();$0",
    ),
    s(
        "mail",
        APP,
        "Mail::to()->send()",
        CODE,
        "Mail::to(${1:\\$user})->send(new ${2:Mailable}(${3}));$0",
    ),
    s(
        "notify",
        APP,
        "Notify a user",
        CODE,
        "${1:\\$user}->notify(new ${2:Notification}(${3}));$0",
    ),
    s(
        "httpget",
        APP,
        "Http::get()",
        CODE,
        "\\$response = Http::${1:acceptJson()->}get('${2:https://}');$0",
    ),
    s(
        "httppost",
        APP,
        "Http::post()",
        CODE,
        "\\$response = Http::post('${1:https://}', [\n\t'${2:key}' => ${3:\\$value},\n]);$0",
    ),
    s(
        "sput",
        APP,
        "Storage::disk()->put()",
        CODE,
        "Storage::disk('${1:public}')->put('${2:path}', ${3:\\$contents});$0",
    ),
    s(
        "logi",
        APP,
        "Log::info() with context",
        CODE,
        "Log::info('${1:message}', [${2}]);$0",
    ),
    s(
        "loge",
        APP,
        "Log::error() with context",
        CODE,
        "Log::error('${1:message}', [${2}]);$0",
    ),
    s(
        "collect",
        APP,
        "collect()->map()",
        CODE,
        "collect(${1:\\$items})->map(fn (${2:\\$item}) => ${3:\\$item})${0}",
    ),
    s(
        "retry",
        APP,
        "retry()",
        CODE,
        "retry(${1:3}, fn () => ${2}, ${3:100});$0",
    ),
    s(
        "rescue",
        APP,
        "rescue()",
        CODE,
        "rescue(fn () => ${1}, ${2:null});$0",
    ),
    s(
        "throwif",
        APP,
        "throw_if()",
        CODE,
        "throw_if(${1:\\$condition}, ${2:\\RuntimeException::class}, '${3:message}');$0",
    ),
    s(
        "test",
        TESTS,
        "PHPUnit test method",
        CLASS,
        "public function test_${1:it_works}(): void\n{\n\t$0\n}",
    ),
    u(
        "testattr",
        TESTS,
        "PHPUnit test method with #[Test]",
        CLASS,
        &["PHPUnit\\Framework\\Attributes\\Test"],
        "#[Test]\npublic function ${1:it_works}(): void\n{\n\t$0\n}",
    ),
    s(
        "setup",
        TESTS,
        "setUp()",
        CLASS,
        "protected function setUp(): void\n{\n\tparent::setUp();\n\n\t$0\n}",
    ),
    s(
        "getok",
        TESTS,
        "GET request that should succeed",
        CODE,
        "\\$this->get('${1:/}')->assertOk();$0",
    ),
    s(
        "postjson",
        TESTS,
        "JSON POST request",
        CODE,
        "\\$this->postJson('${1:/api/path}', [\n\t'${2:key}' => ${3:\\$value},\n])->assert${4:Created}();$0",
    ),
    s(
        "actingas",
        TESTS,
        "actingAs() a factory user",
        CODE,
        "\\$this->actingAs(${1:User}::factory()->create());$0",
    ),
    s(
        "assertdb",
        TESTS,
        "assertDatabaseHas()",
        CODE,
        "\\$this->assertDatabaseHas('${1:table}', [\n\t'${2:column}' => ${3:\\$value},\n]);$0",
    ),
    s(
        "assertmissing",
        TESTS,
        "assertDatabaseMissing()",
        CODE,
        "\\$this->assertDatabaseMissing('${1:table}', [\n\t'${2:column}' => ${3:\\$value},\n]);$0",
    ),
    s(
        "factory",
        TESTS,
        "Model factory create()",
        CODE,
        "\\$${1:user} = ${2:User}::factory()->create(${3});$0",
    ),
    s(
        "factorycount",
        TESTS,
        "Several models from a factory",
        CODE,
        "\\$${1:users} = ${2:User}::factory()->count(${3:3})->create(${4});$0",
    ),
    s(
        "fakeevent",
        TESTS,
        "Event::fake() and assert",
        CODE,
        "Event::fake();\n\n$0\n\nEvent::assertDispatched(${1:Event}::class);",
    ),
    s(
        "fakequeue",
        TESTS,
        "Queue::fake() and assert",
        CODE,
        "Queue::fake();\n\n$0\n\nQueue::assertPushed(${1:Job}::class);",
    ),
    s(
        "fakemail",
        TESTS,
        "Mail::fake() and assert",
        CODE,
        "Mail::fake();\n\n$0\n\nMail::assertSent(${1:Mailable}::class);",
    ),
    s(
        "fakenotification",
        TESTS,
        "Notification::fake() and assert",
        CODE,
        "Notification::fake();\n\n$0\n\nNotification::assertSentTo(${1:\\$user}, ${2:Notification}::class);",
    ),
    s(
        "fakestorage",
        TESTS,
        "Storage::fake()",
        CODE,
        "Storage::fake('${1:public}');$0",
    ),
    s(
        "fakehttp",
        TESTS,
        "Http::fake()",
        CODE,
        "Http::fake([\n\t'${1:example.com/*}' => Http::response(${2:['ok' => true]}, ${3:200}),\n]);$0",
    ),
    s(
        "it",
        PEST,
        "Pest it()",
        CODE,
        "it('${1:does something}', function () {\n\t$0\n});",
    ),
    s(
        "ptest",
        PEST,
        "Pest test()",
        CODE,
        "test('${1:something}', function () {\n\t$0\n});",
    ),
    s(
        "describe",
        PEST,
        "Pest describe()",
        CODE,
        "describe('${1:feature}', function () {\n\t$0\n});",
    ),
    s(
        "beforeeach",
        PEST,
        "Pest beforeEach()",
        CODE,
        "beforeEach(function () {\n\t$0\n});",
    ),
    s(
        "aftereach",
        PEST,
        "Pest afterEach()",
        CODE,
        "afterEach(function () {\n\t$0\n});",
    ),
    s(
        "dataset",
        PEST,
        "Pest dataset()",
        CODE,
        "dataset('${1:emails}', [\n\t${2:'a@example.com'},$0\n]);",
    ),
    s(
        "expect",
        PEST,
        "Pest expect()",
        CODE,
        "expect(${1:\\$value})->${2:toBe}(${3});$0",
    ),
    s(
        "uses",
        PEST,
        "Pest uses()",
        FILE,
        "uses(${1:RefreshDatabase}::class);$0",
    ),
    s(
        "arch",
        PEST,
        "Pest architecture test",
        CODE,
        "arch()\n\t->expect('${1:App}')\n\t->${2:toUseStrictTypes}();$0",
    ),
];

pub struct BladeSnippet {
    pub directive: &'static str,
    pub body: &'static str,
}

pub const BLADE: &[BladeSnippet] = &[
    BladeSnippet {
        directive: "if",
        body: "@if (${1:\\$condition})\n\t$0\n@endif",
    },
    BladeSnippet {
        directive: "unless",
        body: "@unless (${1:\\$condition})\n\t$0\n@endunless",
    },
    BladeSnippet {
        directive: "isset",
        body: "@isset(${1:\\$value})\n\t$0\n@endisset",
    },
    BladeSnippet {
        directive: "empty",
        body: "@empty(${1:\\$value})\n\t$0\n@endempty",
    },
    BladeSnippet {
        directive: "auth",
        body: "@auth\n\t$0\n@endauth",
    },
    BladeSnippet {
        directive: "guest",
        body: "@guest\n\t$0\n@endguest",
    },
    BladeSnippet {
        directive: "foreach",
        body: "@foreach (${1:\\$items} as ${2:\\$item})\n\t$0\n@endforeach",
    },
    BladeSnippet {
        directive: "forelse",
        body: "@forelse (${1:\\$items} as ${2:\\$item})\n\t$0\n@empty\n\t${3:Nothing yet.}\n@endforelse",
    },
    BladeSnippet {
        directive: "for",
        body: "@for (\\$${1:i} = 0; \\$${1:i} < ${2:10}; \\$${1:i}++)\n\t$0\n@endfor",
    },
    BladeSnippet {
        directive: "while",
        body: "@while (${1:\\$condition})\n\t$0\n@endwhile",
    },
    BladeSnippet {
        directive: "switch",
        body: "@switch(${1:\\$value})\n\t@case(${2:1})\n\t\t$0\n\t\t@break\n\t@default\n\t\t\n@endswitch",
    },
    BladeSnippet {
        directive: "section",
        body: "@section('${1:content}')\n\t$0\n@endsection",
    },
    BladeSnippet {
        directive: "push",
        body: "@push('${1:scripts}')\n\t$0\n@endpush",
    },
    BladeSnippet {
        directive: "once",
        body: "@once\n\t$0\n@endonce",
    },
    BladeSnippet {
        directive: "can",
        body: "@can('${1:update}', ${2:\\$model})\n\t$0\n@endcan",
    },
    BladeSnippet {
        directive: "error",
        body: "@error('${1:field}')\n\t<p>{{ \\$message }}</p>$0\n@enderror",
    },
    BladeSnippet {
        directive: "php",
        body: "@php\n\t$0\n@endphp",
    },
    BladeSnippet {
        directive: "verbatim",
        body: "@verbatim\n\t$0\n@endverbatim",
    },
    BladeSnippet {
        directive: "production",
        body: "@production\n\t$0\n@endproduction",
    },
    BladeSnippet {
        directive: "env",
        body: "@env('${1:local}')\n\t$0\n@endenv",
    },
];

pub fn place_at(text: &str, cursor: usize) -> Option<Place> {
    let opened = text[..cursor].rfind("<?php")?;
    let closed = text[..cursor].rfind("?>");
    if closed.is_some_and(|c| c > opened) {
        return None;
    }
    let tokens = tokenize(text, opened + 5, cursor);
    let gap_start = tokens.last().map_or(opened + 5, |t| t.end);
    if tokens
        .last()
        .is_some_and(|t| matches!(&t.kind, Kind::Str { closed: false, .. }) || t.end > cursor)
    {
        return None;
    }
    let gap = &text[gap_start.min(cursor)..cursor];
    let line_comment = gap.contains("//") || gap.replace("#[", "").contains('#');
    let block_comment = gap
        .rfind("/*")
        .is_some_and(|o| gap[o..].find("*/").is_none());
    if line_comment
        && !gap
            .rsplit('\n')
            .next()
            .is_some_and(|l| !l.contains("//") && !l.contains('#'))
        || block_comment
    {
        return None;
    }
    let mut stack: Vec<Place> = Vec::new();
    let mut header_from = 0usize;
    for (i, t) in tokens.iter().enumerate() {
        match t.kind {
            Kind::Open('{') => {
                let header = &tokens[header_from..i];
                let declares = header.iter().enumerate().any(|(k, h)| {
                    matches!(&h.kind, Kind::Ident(w) if matches!(w.as_str(), "class" | "interface" | "trait" | "enum"))
                        && !matches!(k.checked_sub(1).map(|p| &header[p].kind), Some(Kind::DoubleColon))
                });
                stack.push(if declares { Place::Class } else { Place::Code });
                header_from = i + 1;
            }
            Kind::Close('}') => {
                stack.pop();
                header_from = i + 1;
            }
            Kind::Semicolon => header_from = i + 1,
            _ => {}
        }
    }
    Some(stack.last().copied().unwrap_or(Place::File))
}

pub fn word_at(text: &str, cursor: usize) -> Option<usize> {
    let start = text[..cursor]
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map_or(0, |(i, c)| i + c.len_utf8());
    if start == cursor || text[start..cursor].starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let before = text[..start].trim_end_matches([' ', '\t']);
    let prev = text[..start].chars().last();
    if prev.is_some_and(|c| matches!(c, '$' | '>' | ':' | '\\' | '@' | '.' | '-'))
        || [
            "function",
            "new",
            "class",
            "extends",
            "implements",
            "fn",
            "const",
            "case",
            "enum",
            "interface",
            "trait",
        ]
        .iter()
        .any(|w| before.ends_with(w) && before.len() < start)
    {
        return None;
    }
    Some(start)
}

pub fn plain(body: &str) -> String {
    let mut out = String::new();
    let b: Vec<char> = body.chars().collect();
    let mut i = 0;
    let mut fields = 0usize;
    while i < b.len() {
        match b[i] {
            '\\' if i + 1 < b.len() && matches!(b[i + 1], '$' | '}' | '\\') => {
                out.push(b[i + 1]);
                i += 2;
            }
            '$' if i + 1 < b.len() && b[i + 1].is_ascii_digit() => {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            '$' if i + 1 < b.len() && b[i + 1] == '{' => {
                fields += 1;
                i += 2;
                let name_start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == '_') {
                    i += 1;
                }
                if i < b.len() && b[i] == ':' {
                    i += 1;
                } else if !b[name_start].is_ascii_digit() {
                    out.extend(&b[name_start..i]);
                }
            }
            '}' if fields > 0 => {
                fields -= 1;
                i += 1;
            }
            '\t' => {
                out.push_str("    ");
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out.replace("TM_FILENAME_BASE", "ClassName")
}

pub(crate) fn import_edits(text: &str, imports: &[&str]) -> Vec<TextEdit> {
    let missing: Vec<&&str> = imports
        .iter()
        .filter(|fqn| {
            let short = fqn.rsplit('\\').next().unwrap_or(fqn);
            !text.lines().any(|l| {
                let l = l.trim().trim_end_matches(';');
                l == format!("use {fqn}")
                    || l.ends_with(&format!("\\{short}")) && l.starts_with("use ")
            })
        })
        .collect();
    if missing.is_empty() {
        return Vec::new();
    }
    let anchor = text
        .lines()
        .scan(0usize, |at, line| {
            let start = *at;
            *at += line.len() + 1;
            Some((start, line))
        })
        .take_while(|(_, l)| {
            let l = l.trim_start();
            !(l.starts_with("class ")
                || l.starts_with("final ")
                || l.starts_with("abstract ")
                || l.starts_with("return ")
                || l.starts_with("readonly "))
        })
        .filter(|(_, l)| {
            let l = l.trim_start();
            l.starts_with("use ") || l.starts_with("namespace ") || l.starts_with("<?php")
        })
        .last();
    let Some((start, line)) = anchor else {
        return Vec::new();
    };
    let at = (start + line.len()).min(text.len());
    let block: String = missing.iter().map(|fqn| format!("\nuse {fqn};")).collect();
    let text_to_insert = if line.trim_start().starts_with("use ") {
        block
    } else {
        format!("\n{block}")
    };
    let pos = openidea_plugin::text::position(text, at);
    vec![TextEdit {
        range: Range::new(pos, pos),
        text: text_to_insert,
    }]
}

pub fn items(text: &str, cursor: usize, replace: Range) -> Vec<CompletionItem> {
    let Some(start) = word_at(text, cursor) else {
        return Vec::new();
    };
    let bare = text[..start].trim().is_empty();
    let place = place_at(text, cursor);
    if place.is_none() && !bare {
        return Vec::new();
    }
    let typed = text[start..cursor].to_ascii_lowercase();
    SNIPPETS
        .iter()
        .filter(|s| s.group != RELATION)
        .filter(|s| match place {
            Some(place) => s.places.contains(&place) && s.prefix != "php",
            None => s.prefix == "php",
        })
        .filter(|s| s.prefix.starts_with(&typed))
        .map(|s| CompletionItem {
            detail: Some(format!("{} · {}", s.group, s.description)),
            insert_text: s.body.to_owned(),
            snippet: true,
            replace: Some(replace),
            sort_text: Some(format!("~{}", s.prefix)),
            filter_text: Some(s.prefix.to_owned()),
            documentation: Some(format!("```php\n{}\n```", plain(s.body))),
            additional_edits: import_edits(text, s.imports),
            ..CompletionItem::new(s.prefix, CompletionKind::Snippet)
        })
        .collect()
}

pub fn blade_items(
    typed: &str,
    replace: Range,
    available: impl Fn(&str) -> bool,
) -> Vec<CompletionItem> {
    let typed = typed.to_ascii_lowercase();
    BLADE
        .iter()
        .filter(|b| b.directive.starts_with(&typed) && available(b.directive))
        .map(|b| CompletionItem {
            detail: Some(format!("@{} … block", b.directive)),
            insert_text: b.body.to_owned(),
            snippet: true,
            replace: Some(replace),
            sort_text: Some(format!("~{}", b.directive)),
            filter_text: Some(format!("@{}", b.directive)),
            documentation: Some(format!("```blade\n{}\n```", plain(b.body))),
            ..CompletionItem::new(&format!("@{} … @end", b.directive), CompletionKind::Snippet)
        })
        .collect()
}

#[cfg(test)]
fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

#[cfg(test)]
pub fn markdown() -> String {
    let mut out = String::from(
        "# Snippets\n\nType a prefix and pick it from the completion list (or press Tab) to expand it. The placeholders are visited with Tab, and `use` statements the snippet needs are added at the top of the file.\n\nThis file is generated from `src/snippets.rs` by `UPDATE_SNIPPETS=1 cargo test snippets_doc`; edit the table there.\n\n**Where they work**: *file* is outside any class (route files, Pest tests, scripts), *class* is a class, trait or enum body, *code* is inside a function or at the top level of a file.\n",
    );
    let mut groups: Vec<&str> = Vec::new();
    for s in SNIPPETS {
        if !groups.contains(&s.group) {
            groups.push(s.group);
        }
    }
    for group in groups {
        out.push_str(&format!("\n## {group}\n\n"));
        if group == RELATION {
            out.push_str("In a model, these prefixes expand to the relations your migrations suggest: `belongsTo` from the table's `*_id` columns, `hasMany` and `hasOne` from the tables that point at this one, `belongsToMany` from pivot tables, `morphTo` from `*_type` and `*_id` pairs, and `morphMany` and `morphOne` from `*able` columns, each with the real model and method name (relations the model already has are left out). When nothing fits, the snippet offers a choice of the project's models. The table shows the shape they take.\n\n");
        }
        out.push_str("| Prefix | Inserts | Where |\n| --- | --- | --- |\n");
        for s in SNIPPETS.iter().filter(|s| s.group == group) {
            let places = if s.places == CODE {
                "file, code".to_owned()
            } else {
                s.places
                    .iter()
                    .map(|p| match p {
                        Place::File => "file",
                        Place::Class => "class",
                        Place::Code => "code",
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let first = plain(s.body);
            let first = first.lines().next().unwrap_or_default().trim().to_owned();
            out.push_str(&format!(
                "| `{}` | {} — `{}` | {} |\n",
                s.prefix,
                escape_cell(s.description),
                escape_cell(&first),
                places
            ));
        }
    }
    out.push_str("\n## Blade blocks\n\nAfter `@` in a Blade file, block directives also come as a snippet that writes the closing directive.\n\n| Directive | Inserts |\n| --- | --- |\n");
    for b in BLADE {
        let body = plain(b.body).replace('\n', " ⏎ ");
        out.push_str(&format!(
            "| `@{}` | `{}` |\n",
            b.directive,
            escape_cell(body.trim())
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(text: &str) -> Option<Place> {
        place_at(text, text.len())
    }

    #[test]
    fn knows_where_the_cursor_is() {
        assert_eq!(place("<?php\n\nRoute::"), Some(Place::File));
        assert_eq!(place("<?php\nclass A\n{\n    "), Some(Place::Class));
        assert_eq!(
            place("<?php\nclass A\n{\n    public function b()\n    {\n        "),
            Some(Place::Code)
        );
        assert_eq!(place("<?php\nenum A: string\n{\n    "), Some(Place::Class));
        assert_eq!(place("<?php\n$x = new class {\n    "), Some(Place::Class));
        assert_eq!(place("<?php\n$x = 'pub"), None);
        assert_eq!(place("<?php\n// pub"), None);
        assert_eq!(place("<?php\n/* pub"), None);
        assert_eq!(place("<?php\n/* x */ "), Some(Place::File));
        assert_eq!(place("<html>pub"), None);
        assert_eq!(
            place("<?php\n#[Attr]\nclass A\n{\n    "),
            Some(Place::Class)
        );
    }

    #[test]
    fn only_complete_plain_words() {
        assert_eq!(word_at("x pubf", 6), Some(2));
        assert_eq!(word_at("$this->pubf", 11), None);
        assert_eq!(word_at("Foo::pubf", 9), None);
        assert_eq!(word_at("$pubf", 5), None);
        assert_eq!(word_at("public function pubf", 20), None);
        assert_eq!(word_at("x ", 2), None);
        assert_eq!(word_at("// ação ç pubf", 17), Some(13));
        assert_eq!(word_at("çpubf", 6), Some(2));
    }

    #[test]
    fn expands_by_context_and_adds_missing_imports() {
        let text = "<?php\nnamespace App\\Models;\n\nuse Illuminate\\Database\\Eloquent\\Model;\n\nclass Post extends Model\n{\n    scop\n}\n";
        let cursor = text.find("scop").unwrap() + 4;
        let found = items(text, cursor, Range::default());
        let labels: Vec<&str> = found.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["scope", "scopeattr"]);
        assert!(found[0].snippet);
        assert_eq!(
            found[0].additional_edits[0].text,
            "\nuse Illuminate\\Database\\Eloquent\\Builder;"
        );
        let relations = items(
            text.replace("scop", "hasm").as_str(),
            cursor,
            Range::default(),
        );
        assert!(relations.is_empty(), "relations come from the migrations");
        let text = "<?php\nclass A\n{\n    pub\n}\n";
        let labels: Vec<String> = items(text, text.find("pub").unwrap() + 3, Range::default())
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(labels, ["pubf", "pubsf", "pubp", "pubro"]);
        let code = "<?php\nfunction a()\n{\n    fore\n}\n";
        let labels: Vec<String> = items(code, code.find("fore").unwrap() + 4, Range::default())
            .into_iter()
            .map(|i| i.label)
            .collect();
        assert_eq!(labels, ["foreach", "foreachk", "foreign"]);
    }

    #[test]
    fn renders_plain_previews() {
        assert_eq!(
            plain("public function ${1:name}(${2}): ${3:void}\n{\n\t\\$this->${4:x};$0\n}"),
            "public function name(): void\n{\n    $this->x;\n}"
        );
        assert_eq!(plain("class ${1:${TM_FILENAME_BASE}}"), "class ClassName");
    }

    #[test]
    fn prefixes_are_unique_and_bodies_balanced() {
        let mut seen = std::collections::BTreeSet::new();
        for s in SNIPPETS {
            assert!(seen.insert(s.prefix), "duplicate prefix {}", s.prefix);
            assert_eq!(s.prefix, s.prefix.to_ascii_lowercase(), "{}", s.prefix);
            let opened = s.body.matches("${").count();
            let closed = s.body.matches('}').count() - s.body.matches("\\}").count();
            assert!(closed >= opened, "{} has unbalanced placeholders", s.prefix);
        }
    }

    #[test]
    fn snippets_doc_is_up_to_date() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/snippets.md");
        let generated = markdown();
        if std::env::var("UPDATE_SNIPPETS").is_ok() {
            std::fs::write(path, &generated).unwrap();
        }
        let current = std::fs::read_to_string(path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            current == generated,
            "docs/snippets.md is out of date: run UPDATE_SNIPPETS=1 cargo test snippets_doc"
        );
    }
}
