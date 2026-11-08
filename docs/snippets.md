# Snippets

Type a prefix and pick it from the completion list (or press Tab) to expand it. The placeholders are visited with Tab, and `use` statements the snippet needs are added at the top of the file.

This file is generated from `src/snippets.rs` by `UPDATE_SNIPPETS=1 cargo test snippets_doc`; edit the table there.

**Where they work**: *file* is outside any class (route files, Pest tests, scripts), *class* is a class, trait or enum body, *code* is inside a function or at the top level of a file.

## PHP

| Prefix | Inserts | Where |
| --- | --- | --- |
| `php` | PHP opening tag — `<?php` | file |
| `strict` | declare(strict_types=1); — `declare(strict_types=1);` | file |
| `class` | Class named after the file — `class ClassName` | file |
| `fclass` | Final class — `final class ClassName` | file |
| `aclass` | Abstract class — `abstract class ClassName` | file |
| `rclass` | Readonly class — `readonly class ClassName` | file |
| `cext` | Class that extends another — `class ClassName extends Parent` | file |
| `interface` | Interface — `interface ClassName` | file |
| `trait` | Trait — `trait ClassName` | file |
| `enum` | Pure enum — `enum ClassName` | file |
| `enums` | String-backed enum — `enum ClassName: string` | file |
| `enumi` | Int-backed enum — `enum ClassName: int` | file |
| `func` | Function declaration — `function name(): void` | file |

## Class members

| Prefix | Inserts | Where |
| --- | --- | --- |
| `pubf` | public function — `public function name(): void` | class |
| `prof` | protected function — `protected function name(): void` | class |
| `prif` | private function — `private function name(): void` | class |
| `pubsf` | public static function — `public static function name(): void` | class |
| `prosf` | protected static function — `protected static function name(): void` | class |
| `prisf` | private static function — `private static function name(): void` | class |
| `absf` | abstract function — `abstract public function name(): void;` | class |
| `ctor` | Constructor — `public function __construct()` | class |
| `ctorp` | Constructor with promoted properties — `public function __construct(` | class |
| `pubp` | public property — `public string $name;` | class |
| `prop` | protected property — `protected string $name;` | class |
| `prip` | private property — `private string $name;` | class |
| `pubro` | public readonly property — `public readonly string $name;` | class |
| `const` | Class constant — `public const NAME = 'value';` | class |
| `case` | Enum case with a value — `case Name = 'value';` | class |
| `get` | Getter — `public function getName(): mixed` | class |
| `set` | Fluent setter — `public function setName(mixed $value): static` | class |
| `invoke` | __invoke() — `public function __invoke(): mixed` | class |
| `tostring` | __toString() — `public function __toString(): string` | class |
| `magicget` | __get() — `public function __get(string $name): mixed` | class |
| `magicset` | __set() — `public function __set(string $name, mixed $value): void` | class |
| `magiccall` | __call() — `public function __call(string $method, array $arguments): mixed` | class |
| `magiccallstatic` | __callStatic() — `public static function __callStatic(string $method, array $arguments): mixed` | class |

## Control flow

| Prefix | Inserts | Where |
| --- | --- | --- |
| `if` | if — `if ($condition) {` | file, code |
| `ifel` | if … else — `if ($condition) {` | file, code |
| `elif` | elseif — `elseif ($condition) {` | file, code |
| `else` | else — `else {` | file, code |
| `foreach` | foreach — `foreach ($items as $item) {` | file, code |
| `foreachk` | foreach with key — `foreach ($items as $key => $value) {` | file, code |
| `for` | for — `for ($i = 0; $i < $count; $i++) {` | file, code |
| `while` | while — `while ($condition) {` | file, code |
| `do` | do … while — `do {` | file, code |
| `switch` | switch — `switch ($value) {` | file, code |
| `match` | match expression — `match ($value) {` | file, code |
| `try` | try … catch — `try {` | file, code |
| `tryf` | try … finally — `try {` | file, code |
| `trycf` | try … catch … finally — `try {` | file, code |
| `throw` | throw new exception — `throw new \RuntimeException('message');` | file, code |
| `fn` | Arrow function — `fn ($item) => $item` | file, code |
| `closure` | Closure with use — `function () use () {` | file, code |
| `ret` | return — `return ;` | file, code |

## Laravel helpers

| Prefix | Inserts | Where |
| --- | --- | --- |
| `dd` | dd() — `dd();` | file, code |
| `dump` | dump() — `dump();` | file, code |
| `validate` | $request->validate() — `$validated = $request->validate([` | file, code |
| `validator` | Validator::make() — `$validator = Validator::make($data, [` | file, code |
| `retview` | return view() — `return view('name', [` | file, code |
| `retjson` | return response()->json() — `return response()->json($data, 200);` | file, code |
| `retredirect` | return redirect()->route() — `return redirect()->route('name')->with('status', 'Saved');` | file, code |
| `back` | return back() with a message — `return back()->with('status', 'Saved');` | file, code |
| `abortif` | abort_if() — `abort_if($condition, 403);` | file, code |
| `abortunless` | abort_unless() — `abort_unless($condition, 403);` | file, code |
| `gate` | Gate::authorize() — `Gate::authorize('update', $model);` | file, code |
| `authuser` | The signed-in user — `$user = auth()->user();` | file, code |
| `cremember` | Cache::remember() — `Cache::remember('key', now()->addMinutes(10), fn () => $value);` | file, code |
| `cforever` | Cache::rememberForever() — `Cache::rememberForever('key', fn () => $value);` | file, code |
| `cforget` | Cache::forget() — `Cache::forget('key');` | file, code |
| `clock` | Cache::lock() — `Cache::lock('key', 10)->get(function () {` | file, code |
| `dispatch` | Dispatch a job — `Job::dispatch();` | file, code |
| `event` | Fire an event — `event(new Event());` | file, code |
| `chain` | Bus::chain() — `Bus::chain([` | file, code |
| `mail` | Mail::to()->send() — `Mail::to($user)->send(new Mailable());` | file, code |
| `notify` | Notify a user — `$user->notify(new Notification());` | file, code |
| `httpget` | Http::get() — `$response = Http::acceptJson()->get('https://');` | file, code |
| `httppost` | Http::post() — `$response = Http::post('https://', [` | file, code |
| `sput` | Storage::disk()->put() — `Storage::disk('public')->put('path', $contents);` | file, code |
| `logi` | Log::info() with context — `Log::info('message', []);` | file, code |
| `loge` | Log::error() with context — `Log::error('message', []);` | file, code |
| `collect` | collect()->map() — `collect($items)->map(fn ($item) => $item)` | file, code |
| `retry` | retry() — `retry(3, fn () => , 100);` | file, code |
| `rescue` | rescue() — `rescue(fn () => , null);` | file, code |
| `throwif` | throw_if() — `throw_if($condition, \RuntimeException::class, 'message');` | file, code |

## Eloquent models

| Prefix | Inserts | Where |
| --- | --- | --- |
| `fillable` | $fillable — `protected $fillable = [` | class |
| `guarded` | $guarded — `protected $guarded = ['id'];` | class |
| `hidden` | $hidden — `protected $hidden = [` | class |
| `visible` | $visible — `protected $visible = [` | class |
| `appends` | $appends — `protected $appends = [` | class |
| `with` | $with (always eager load) — `protected $with = ['relation'];` | class |
| `table` | $table — `protected $table = 'table';` | class |
| `primarykey` | $primaryKey and key type — `protected $primaryKey = 'uuid';` | class |
| `hasfactory` | use HasFactory — `use HasFactory;` | class |
| `timestamps` | Turn timestamps off — `public $timestamps = false;` | class |
| `connection` | $connection — `protected $connection = 'mysql';` | class |
| `perpage` | $perPage — `protected $perPage = 25;` | class |
| `touches` | $touches — `protected $touches = ['post'];` | class |
| `casts` | casts() method — `protected function casts(): array` | class |
| `castsp` | $casts property (Laravel 10 and older) — `protected $casts = [` | class |
| `scope` | Local scope — `public function scopeActive(Builder $query): void` | class |
| `scopeattr` | Local scope with #[Scope] (Laravel 12) — `#[Scope]` | class |
| `accessor` | Accessor (Attribute::make get) — `protected function name(): Attribute` | class |
| `mutator` | Mutator (Attribute::make set) — `protected function name(): Attribute` | class |
| `attr` | Accessor and mutator — `protected function name(): Attribute` | class |
| `booted` | booted() with a model event — `protected static function booted(): void` | class |

## Eloquent relations

| Prefix | Inserts | Where |
| --- | --- | --- |
| `hasone` | hasOne relation — `public function profile(): HasOne` | class |
| `hasmany` | hasMany relation — `public function posts(): HasMany` | class |
| `belongsto` | belongsTo relation — `public function user(): BelongsTo` | class |
| `belongstomany` | belongsToMany relation — `public function roles(): BelongsToMany` | class |
| `hasonethrough` | hasOneThrough relation — `public function owner(): HasOneThrough` | class |
| `hasmanythrough` | hasManyThrough relation — `public function deployments(): HasManyThrough` | class |
| `morphto` | morphTo relation — `public function commentable(): MorphTo` | class |
| `morphone` | morphOne relation — `public function image(): MorphOne` | class |
| `morphmany` | morphMany relation — `public function comments(): MorphMany` | class |
| `morphtomany` | morphToMany relation — `public function tags(): MorphToMany` | class |
| `morphedbymany` | morphedByMany relation — `public function posts(): MorphToMany` | class |

## Eloquent and queries

| Prefix | Inserts | Where |
| --- | --- | --- |
| `qwhere` | Query with where() — `Model::query()` | file, code |
| `qfirst` | First matching model or fail — `Model::where('column', $value)->firstOrFail();` | file, code |
| `qfind` | findOrFail() — `$model = Model::findOrFail($id);` | file, code |
| `qcreate` | create() — `Model::create([` | file, code |
| `qupdate` | update() on a model — `$model->update([` | file, code |
| `qfoc` | firstOrCreate() — `Model::firstOrCreate(` | file, code |
| `quoc` | updateOrCreate() — `Model::updateOrCreate(` | file, code |
| `qwith` | Eager load with with() — `Model::with('relation')->get();` | file, code |
| `qwherehas` | whereHas() — `->whereHas('relation', fn ($query) => $query->where('column', $value))` | file, code |
| `qpaginate` | Paginated query — `Model::query()` | file, code |
| `qchunk` | chunkById() — `Model::query()->chunkById(200, function ($models) {` | file, code |
| `qlazy` | lazy() iteration — `foreach (Model::query()->lazy() as $model) {` | file, code |
| `dbtrans` | DB::transaction() — `DB::transaction(function () {` | file, code |
| `dbtable` | DB::table() query — `DB::table('table')` | file, code |
| `dbselect` | DB::select() with bindings — `DB::select('select * from users where id = ?', [$id]);` | file, code |

## Laravel classes

| Prefix | Inserts | Where |
| --- | --- | --- |
| `index` | Controller index() — `public function index()` | class |
| `create` | Controller create() — `public function create()` | class |
| `store` | Controller store() — `public function store(Request $request)` | class |
| `show` | Controller show() — `public function show(Model $model)` | class |
| `edit` | Controller edit() — `public function edit(Model $model)` | class |
| `update` | Controller update() — `public function update(Request $request, Model $model)` | class |
| `destroy` | Controller destroy() — `public function destroy(Model $model)` | class |
| `resource` | The seven resource controller methods — `public function index()` | class |
| `authorize` | Form Request authorize() — `public function authorize(): bool` | class |
| `rules` | Form Request rules() — `public function rules(): array` | class |
| `messages` | Form Request messages() — `public function messages(): array` | class |
| `prepare` | prepareForValidation() — `protected function prepareForValidation(): void` | class |
| `handle` | handle() for jobs, listeners and commands — `public function handle(): void` | class |
| `failed` | Job failed() — `public function failed(?\Throwable $exception): void` | class |
| `tries` | Job $tries and $backoff — `public $tries = 3;` | class |
| `signature` | Artisan command signature and description — `protected $signature = 'app:command {argument} {--option}';` | class |
| `mhandle` | Middleware handle() — `public function handle(Request $request, Closure $next): Response` | class |
| `policy` | Policy methods — `public function viewAny(User $user): bool` | class |
| `register` | Service provider register() — `public function register(): void` | class |
| `boot` | Service provider boot() — `public function boot(): void` | class |
| `via` | Notification via() — `public function via(object $notifiable): array` | class |
| `tomail` | Notification toMail() — `public function toMail(object $notifiable): MailMessage` | class |
| `toarray` | toArray() for notifications and resources — `public function toArray(object $notifiable): array` | class |
| `envelope` | Mailable envelope() and content() — `public function envelope(): Envelope` | class |
| `definition` | Factory definition() — `public function definition(): array` | class |
| `state` | Factory state — `public function suspended(): static` | class |
| `run` | Seeder run() — `public function run(): void` | class |
| `render` | render() for components — `public function render()` | class |
| `observer` | Observer events — `public function created(Model $model): void` | class |

## Migrations

| Prefix | Inserts | Where |
| --- | --- | --- |
| `up` | up() creating a table — `public function up(): void` | class |
| `uptable` | up() changing a table — `public function up(): void` | class |
| `down` | down() dropping a table — `public function down(): void` | class |
| `migration` | Anonymous migration class — `return new class extends Migration` | file |
| `foreign` | foreignId()->constrained() — `$table->foreignId('user_id')->constrained()->cascadeOnDelete();` | file, code |

## Routes

| Prefix | Inserts | Where |
| --- | --- | --- |
| `rget` | Route::get() — `Route::get('/path', [Controller::class, 'index'])->name('name');` | file, code |
| `rpost` | Route::post() — `Route::post('/path', [Controller::class, 'store'])->name('name');` | file, code |
| `rput` | Route::put() — `Route::put('/path', [Controller::class, 'update'])->name('name');` | file, code |
| `rpatch` | Route::patch() — `Route::patch('/path', [Controller::class, 'update'])->name('name');` | file, code |
| `rdelete` | Route::delete() — `Route::delete('/path', [Controller::class, 'destroy'])->name('name');` | file, code |
| `rview` | Route::view() — `Route::view('/path', 'view')->name('name');` | file, code |
| `rredirect` | Route::redirect() — `Route::redirect('/here', '/there');` | file, code |
| `rresource` | Route::resource() — `Route::resource('photos', PhotoController::class);` | file, code |
| `rapi` | Route::apiResource() — `Route::apiResource('photos', PhotoController::class);` | file, code |
| `rgroup` | Route group with middleware — `Route::middleware(['auth'])->group(function () {` | file, code |
| `rprefix` | Route group with prefix and name — `Route::prefix('admin')->name('admin.')->group(function () {` | file, code |
| `rcontroller` | Route::controller() group — `Route::controller(Controller::class)->group(function () {` | file, code |
| `rfallback` | Route::fallback() — `Route::fallback(function () {` | file, code |

## Tests (PHPUnit)

| Prefix | Inserts | Where |
| --- | --- | --- |
| `test` | PHPUnit test method — `public function test_it_works(): void` | class |
| `testattr` | PHPUnit test method with #[Test] — `#[Test]` | class |
| `setup` | setUp() — `protected function setUp(): void` | class |
| `getok` | GET request that should succeed — `$this->get('/')->assertOk();` | file, code |
| `postjson` | JSON POST request — `$this->postJson('/api/path', [` | file, code |
| `actingas` | actingAs() a factory user — `$this->actingAs(User::factory()->create());` | file, code |
| `assertdb` | assertDatabaseHas() — `$this->assertDatabaseHas('table', [` | file, code |
| `assertmissing` | assertDatabaseMissing() — `$this->assertDatabaseMissing('table', [` | file, code |
| `factory` | Model factory create() — `$user = User::factory()->create();` | file, code |
| `factorycount` | Several models from a factory — `$users = User::factory()->count(3)->create();` | file, code |
| `fakeevent` | Event::fake() and assert — `Event::fake();` | file, code |
| `fakequeue` | Queue::fake() and assert — `Queue::fake();` | file, code |
| `fakemail` | Mail::fake() and assert — `Mail::fake();` | file, code |
| `fakenotification` | Notification::fake() and assert — `Notification::fake();` | file, code |
| `fakestorage` | Storage::fake() — `Storage::fake('public');` | file, code |
| `fakehttp` | Http::fake() — `Http::fake([` | file, code |

## Pest

| Prefix | Inserts | Where |
| --- | --- | --- |
| `it` | Pest it() — `it('does something', function () {` | file, code |
| `ptest` | Pest test() — `test('something', function () {` | file, code |
| `describe` | Pest describe() — `describe('feature', function () {` | file, code |
| `beforeeach` | Pest beforeEach() — `beforeEach(function () {` | file, code |
| `aftereach` | Pest afterEach() — `afterEach(function () {` | file, code |
| `dataset` | Pest dataset() — `dataset('emails', [` | file, code |
| `expect` | Pest expect() — `expect($value)->toBe();` | file, code |
| `uses` | Pest uses() — `uses(RefreshDatabase::class);` | file |
| `arch` | Pest architecture test — `arch()` | file, code |

## Blade blocks

After `@` in a Blade file, block directives also come as a snippet that writes the closing directive.

| Directive | Inserts |
| --- | --- |
| `@if` | `@if ($condition) ⏎      ⏎ @endif` |
| `@unless` | `@unless ($condition) ⏎      ⏎ @endunless` |
| `@isset` | `@isset($value) ⏎      ⏎ @endisset` |
| `@empty` | `@empty($value) ⏎      ⏎ @endempty` |
| `@auth` | `@auth ⏎      ⏎ @endauth` |
| `@guest` | `@guest ⏎      ⏎ @endguest` |
| `@foreach` | `@foreach ($items as $item) ⏎      ⏎ @endforeach` |
| `@forelse` | `@forelse ($items as $item) ⏎      ⏎ @empty ⏎     Nothing yet. ⏎ @endforelse` |
| `@for` | `@for ($i = 0; $i < 10; $i++) ⏎      ⏎ @endfor` |
| `@while` | `@while ($condition) ⏎      ⏎ @endwhile` |
| `@switch` | `@switch($value) ⏎     @case(1) ⏎          ⏎         @break ⏎     @default ⏎          ⏎ @endswitch` |
| `@section` | `@section('content') ⏎      ⏎ @endsection` |
| `@push` | `@push('scripts') ⏎      ⏎ @endpush` |
| `@once` | `@once ⏎      ⏎ @endonce` |
| `@can` | `@can('update', $model) ⏎      ⏎ @endcan` |
| `@error` | `@error('field') ⏎     <p>{{ $message }}</p> ⏎ @enderror` |
| `@php` | `@php ⏎      ⏎ @endphp` |
| `@verbatim` | `@verbatim ⏎      ⏎ @endverbatim` |
| `@production` | `@production ⏎      ⏎ @endproduction` |
| `@env` | `@env('local') ⏎      ⏎ @endenv` |
