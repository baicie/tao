# Nexa Language Core v0.6

## Status

Language Core v0.6 is delivered and remains a normative predecessor. Its
implementation satisfies the exit criteria in the
[v0.6 roadmap](../project/roadmap/language-core-v0.6.md). The current executable
language is [Language Core v0.8](language-core-v0.8.md).

## Product Contract

Language Core v0.6 extends v0.5 with explicit multi-file modules. One entry
source and every source reachable through a valid relative import form one
compiler session. A module may export top-level functions, records, and tagged
unions; another module may import those declarations by exact name.

Modules do not add JavaScript or TypeScript module semantics. There are no
module objects, package lookup, implicit index files, runtime loading, or
top-level initialization effects. The CLI owns source access, the compiler
session owns the source map and module graph, and each parser still consumes
exactly one already loaded UTF-8 source.

The newly reserved words are `import`, `export`, and `from`. A v0.5 program
that used one of those words as an identifier is not source-compatible with
v0.6. All other accepted v0.5 programs retain their behavior.

## Accepted Program

`math.nexa` exports a nominal type and a function:

```nexa
export type Pair = {
  left: Int;
  right: Int;
};

export function add(pair: Pair): Int {
  return pair.left + pair.right;
}
```

`main.nexa` is the entry module:

```nexa
import { Pair, add } from "./math.nexa";

function main(): Unit {
  const pair: Pair = { left: 20, right: 22 };
  print(add(pair));
}
```

`nexac check main.nexa` succeeds and `nexac run main.nexa` prints `42`.
The export modifier does not change how a declaration is used inside its own
module.

## Grammar

Whitespace and `//` line comments may appear between grammar symbols and
remain in the lossless CST. These productions replace or extend the
corresponding v0.5 productions; all other v0.5 grammar remains unchanged.

```text
SourceFile             = TopLevelItem*
TopLevelItem           = ImportDeclaration
                       | ExportDeclaration
                       | Declaration
ImportDeclaration      = "import" "{" ImportNameList "}"
                         "from" StringLiteral ";"
ImportNameList         = Identifier ("," Identifier)*
ExportDeclaration      = "export" ExportableDeclaration
ExportableDeclaration  = FunctionDeclaration
                       | RecordDeclaration
                       | UnionDeclaration
Declaration            = FunctionDeclaration
                       | RecordDeclaration
                       | UnionDeclaration
```

An import may occur anywhere among top-level items. Its bindings are
module-scoped, so textual placement does not limit their visibility. An import
list contains at least one identifier and never accepts a trailing comma.
Every import ends in a semicolon.

`export` is a modifier on one following function, record, or union declaration.
It cannot modify an import or appear on a statement or local binding. The
v0.5 `type` disambiguation after `=` continues to distinguish records from
tagged unions.

Malformed delimiters, an empty import list, a trailing import comma, a missing
`from`, path literal, semicolon, or exportable declaration use `E1001`. Parser
recovery must retain later import and declaration nodes without loading a
source or performing visibility checks.

## Import Path Contract

The decoded contents of every syntactically valid import string must satisfy
all of these rules before the source provider is queried:

- The path begins with exactly `./` or one or more leading `../` components.
- `/` is the only source-level separator. A decoded backslash is invalid on
  every host, including Windows.
- Empty components are invalid. Interior `.` and `..` components are allowed
  and are normalized during resolution.
- The final component has a non-empty stem and ends in the exact lowercase
  extension `.nexa`.
- Absolute paths, drive-qualified paths, bare package names, URLs, NUL bytes,
  query/fragment suffixes, and directory-only paths are invalid.

The compiler does not append `.nexa`, try `index.nexa`, search parent package
directories, inspect a manifest, or fall back to another spelling. It does not
percent-decode or Unicode-normalize an import string. `./data.NEXA`,
`./folder/`, `library`, `C:/library.nexa`, and `.\\library.nexa` are all
invalid; `./library.nexa`, `../shared/library.nexa`, and
`./parts/../library.nexa` are valid spellings.

A valid spelling is resolved relative to the importing module's logical
location. Leading or interior `..` components may move above the entry file's
directory; any access restriction is a source-provider policy and a rejected
load still uses `E4001`. Import paths and resolved host paths are not values
observable by a running Nexa program.

An invalid spelling, a target that cannot be resolved or does not exist, an
unreadable target, and a target whose bytes are not valid UTF-8 all use
`E4001`. The primary label covers the complete import `StringLiteral`,
including quotes. A malformed string token or escape remains `E1001`, not
`E4001`. Failure to read the command-line entry file occurs before a source
span exists and remains a CLI I/O error rather than a synthetic `E4001`.

## Source Provider And Compiler Session

The CLI supplies a source provider to a compiler session. The provider has
three conceptual responsibilities:

1. identify the entry source with an opaque, equality-comparable `SourceKey`;
2. resolve a validated import spelling against an importing `SourceKey` to
   another canonical `SourceKey`; and
3. load bytes and a diagnostic display path for a `SourceKey`.

The exact Rust trait shape is an implementation detail, but these behaviors
are required. The compiler validates UTF-8, registers each successfully loaded
source in one `SourceMap`, and gives a parser only `(FileId, &str)`. Parsers,
HIR, MIR, and the interpreter never open files. Tests must be able to supply an
in-memory provider without host filesystem access.

`SourceKey` is the identity used to deduplicate modules. Equivalent relative
spellings, filesystem aliases recognized by the provider, and diamond imports
that resolve to an equal key all identify one module. A session loads a key at
most once and reuses both its source and semantic results. The provider's
display path is used only for diagnostics; neither it nor the opaque key is a
language value.

Load failures are cached by key. Every distinct import occurrence that
references a failed key receives its own `E4001` at that occurrence, even
though the provider is not asked to load the bytes again. Sources that are not
reachable from the entry graph are neither requested nor checked.

## Module Graph And Stable Order

The entry source is always `ModuleId(0)`. Reachable dependencies are discovered
by a deterministic depth-first traversal:

1. scan a module's import declarations in source order;
2. resolve each import target to a `SourceKey`;
3. when a key is first discovered, allocate the next `ModuleId` and `FileId`,
   then visit that module before scanning the importing module's next import;
4. when a key is already known, add the graph edge and reuse its module.

Names inside one import list do not affect graph order. Multiple declarations
that target one module create multiple import edges but do not create another
module identity.

For this diamond:

```text
entry -> left  -> shared
      -> right -> shared
```

when `left` is imported before `right`, the order is `entry = 0`, `left = 1`,
`shared = 2`, `right = 3`. Changing declaration order may change internal IDs,
but a fixed source graph and provider mapping always produce the same order.
Numeric IDs are not observable language values.

After the reachable graph is loaded, the session computes strongly connected
components. An SCC is cyclic when it contains more than one module or contains
a self-import edge. Every cyclic SCC produces exactly one `E4002`.

The deterministic DFS records the first edge within that SCC that points to an
active ancestor. That closing import edge is the cycle witness's primary label;
the complete path literal is labeled. The DFS-tree import edges from the target
ancestor back to the closing edge's source are secondary labels, in cycle
order. A self-import therefore has only the primary label. Extra edges in a
complex SCC do not create more cycle diagnostics. Independent syntax and
semantic errors in cyclic modules are still reported.

## Visibility And Namespaces

Every top-level function, record, and union is private unless its declaration
has `export`. Private declarations remain fully visible inside their defining
module. Imports expose only explicitly exported declarations and never expose
another module's private implementation.

Each module retains the v0.5 internal namespaces:

- functions and imported functions occupy the top-level value namespace;
- records, unions, and imported records/unions occupy the type namespace; and
- one value and one type may have the same spelling inside a module.

An imported declaration enters the namespace determined by its resolved kind.
It is one module-wide immutable binding to the original definition, not a copy
or a new nominal identity. A same-namespace collision between an import and a
local declaration, or between two imports, uses the existing `E2002`: the
later binding in source order is primary and the first binding is secondary.
Cross-namespace local/import pairs remain legal.

For consumers, a module has one external export namespace shared by functions,
records, and unions. Consequently these two internally legal declarations
cannot both be exported:

```nexa
export type Item = { value: Int; };
export function Item(): Int { return 0; }
```

The later exported name uses `E4005`, with a primary label on its exact name
identifier and a secondary label on the first exported name. Existing
same-namespace duplicate declarations continue to use `E2002` and do not gain
a derivative `E4005`. An invalid export table keeps the first otherwise valid
export as the deterministic recovery binding.

For each imported name, resolution has exactly three outcomes:

- one exported declaration with that name resolves successfully;
- no declaration of either internal namespace has that name uses `E4003`,
  with a primary label on the imported identifier; or
- one or more declarations have that name but none is exported uses `E4004`,
  with a primary label on the imported identifier and a secondary label on the
  earliest matching private declaration name in target source order.

When a matching valid export exists, an additional private declaration with
the same cross-namespace spelling does not hide it. Each missing or private
identifier in a multi-name import receives its own diagnostic.

Imports cannot be aliased and are not exports. Importing a declaration never
re-exports it, and `export import` is malformed syntax. There are no star,
default, namespace, or side-effect-only imports.

## Module-Aware Identities

Every reachable module owns its definitions. The stable top-level identities
have these logical shapes:

```text
FunctionId = { module: ModuleId, index: function source-order index }
RecordId   = { module: ModuleId, index: record source-order index }
UnionId    = { module: ModuleId, index: union source-order index }

DefId = Function(FunctionId) | Record(RecordId) | Union(UnionId)
```

Each index is allocated independently among declarations of the same kind in
that module. There is no shared cross-kind index stream. `DefId` is the
kind-safe sum used by export tables and import resolution, not a lossy numeric
pair. `FieldId`, `VariantId`, and payload identities remain scoped by their
parent `RecordId`/`UnionId` and thereby inherit module ownership.

Imports allocate no definition identity, and `export` does not change one.
Same-kind identities are allocated before semantic duplicate/visibility
rejection so an earlier diagnostic does not renumber unrelated later
definitions of that kind. The `ModuleId(0)` compatibility constructors for
existing single-file IDs may therefore retain their v0.5 meaning.

An imported name resolves directly to the target's `DefId`. A diamond import
therefore preserves one definition and one nominal type identity. Equal record
shapes or union layouts declared in different modules remain distinct types.
Typed HIR records the resolved `DefId`, exact kind, type, and source span for
every import and cross-module reference before MIR lowering.

## Entry Point

Only a function declared locally in `ModuleId(0)` can be the program entry
point. Its export status is irrelevant. It must use one of the existing v0.3
signatures:

```nexa
function main(): Unit {}
function main(args: String[]): Unit {}
```

An invalid entry-module `main` signature retains `E3003`. Ordinary duplicate
declaration rules ensure there cannot be two local entry candidates.

A dependency's function named `main` is an ordinary function. It is not
subject to the special entry signature rule and can never become the entry by
graph order or name scanning. If exported, it may be explicitly imported and
called like any other function. An imported `main` binding is likewise not an
entry candidate; it collides with a local value named `main` under `E2002`.

`nexac check` validates the complete reachable graph but continues to accept
an entry module with no local `main`. `nexac run` requires a resolved local
entry. If none exists, it returns the existing structured
``program has no `main` entry point`` runtime failure before any function runs.
The CLI argument behavior for the two valid signatures is unchanged.

## Evaluation, MIR, And Interpreter Contract

Imports and exports have no runtime evaluation and cannot emit output. All
top-level references are resolved to module-aware identities in typed HIR.
CFG MIR contains resolved definition and layout identities for cross-module
calls, record operations, union construction, and matching. It never performs
source-path or source-name lookup.

`MirProgram` records the entry module and an optional already resolved entry
function identity. The interpreter invokes that identity; it must not search
the flattened function list for the string `main`. It does not receive a
source provider or module namespace.

Cross-module calls preserve the existing left-to-right evaluation rules. The
100000-basic-block step budget, 64-active-call limit, and recursive-union depth
limit are global to one execution, not reset at module boundaries. Runtime
failures retain the `FileId` of the expression in the defining module and
preserve all earlier output.

Malformed programmatic MIR with an unknown module/definition identity, the
wrong definition kind, an invalid entry identity, or an owner/layout mismatch
returns a structured interpreter error rather than panicking. Valid typed HIR
must never lower to such MIR.

`nexac parse file.nexa` remains a one-source CST operation and does not load
imports. `nexac check file.nexa` and `nexac run file.nexa` treat the argument
as the entry and load its reachable graph. Execution begins only when loading,
parsing, resolution, and type checking produce no error diagnostics.

## Required Diagnostics And Labels

v0.6 retains every v0.5 diagnostic and adds `E4001` through `E4005`.

| Code | Condition | Primary label | Secondary label(s) |
|---|---|---|---|
| `E4001` | invalid import path, unresolved/missing source, unreadable source, or non-UTF-8 source | complete import path literal | none |
| `E4002` | cyclic module SCC | complete path literal on the deterministic closing edge | complete path literals on the DFS-tree witness edges, in cycle order |
| `E4003` | target module has no declaration with the imported name | imported identifier | none |
| `E4004` | target has the name but no matching declaration is exported | imported identifier | earliest matching private declaration name |
| `E4005` | function/type export-name collision | later exported declaration name | first exported declaration name |

Same-namespace import/local/import collisions remain `E2002`. Malformed import
or export grammar remains `E1001`; module path validation begins only after a
complete path string token exists.

Diagnostics from every reachable file and compiler phase are merged and
stable-sorted lexicographically by the primary label's
`(FileId, start byte, end byte, diagnostic code)`. Exact ties retain emission
order. Secondary labels do not change diagnostic order. `FileId` allocation
follows first-discovery module order, and UTF-8 line/column rendering continues
to derive from the owning source's byte spans.

## Rejected Programs

A bare or extensionless path is invalid (`E4001`):

```nexa
import { add } from "math";
```

A missing export is distinct from a private declaration. This importer uses
`E4004`, with the private declaration in `math.nexa` as a secondary label:

```nexa
import { add } from "./math.nexa";
```

```nexa
function add(left: Int, right: Int): Int {
  return left + right;
}
```

Importing a name absent from both target namespaces uses `E4003`:

```nexa
import { subtract } from "./math.nexa";
```

A cross-kind external export collision uses `E4005`:

```nexa
export type Value = { inner: Int; };
export function Value(): Int { return 0; }
```

This two-file graph is cyclic (`E4002`):

```nexa
// left.nexa
import { right } from "./right.nexa";
export function left(): Int { return right(); }
```

```nexa
// right.nexa
import { left } from "./left.nexa";
export function right(): Int { return left(); }
```

## Compatibility And Non-Goals

A single v0.5 file is a one-node v0.6 graph with `ModuleId(0)`. Its declarations
remain mutually visible under the existing value/type namespace rules, and its
runtime behavior is unchanged. The only intentional source break is reserving
`import`, `export`, and `from`. Internal numeric identity values may change to
carry module ownership because they are not source-observable.

Language Core v0.6 does not add import aliases, re-exports, export lists, star
or default exports, namespace imports, side-effect imports, dynamic imports,
module values, top-level executable statements, cyclic-module execution,
absolute imports, extension inference, implicit `index.nexa`, package or remote
resolution, manifests, a package manager, conditional compilation, reflection,
generic types/functions, `Option`, `Result`, function values, closures,
exceptions, async execution, native AOT, LLVM, WebAssembly, UI, OXC, SWC, or
TypeScript/JavaScript compatibility.
