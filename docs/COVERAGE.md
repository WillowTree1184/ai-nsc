# Spec coverage matrix

Status against `docs/grammar/Draft 1.153.md` (trial draft). The specification is the
source of truth and evolves; the matrix records what the compiler implements and
what it diagnoses instead of miscompiling.

Legend:

- ✅ implemented and covered by automated tests
- 🟡 implemented with documented limitations
- ⛔ not implemented yet (diagnosed as “not implemented yet”, never silently
  miscompiled)

## Verification

Gates run every round:

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`cargo test` currently runs **402 tests**: 41 lib unit tests (lexer, literal,
mangling, AST clone), 54 CLI behaviour tests, 2 LLVM codegen tests, 125
end-to-end execution programs, 10 parser tests and **2 spec-conformance tests**
that read `docs/grammar/Draft 1.153.md` directly (a closed-set test also asserts that the only spec programs failing semantic analysis are the documented demonstration/inconsistency snippets):

1. every `novasharp` fence in the specification parses (80+ examples);
2. every whole-program example that passes semantic analysis is compiled end to
   end by the CLI to an executable without diagnostics (15+ examples; examples
   with elided bodies or deliberately invalid code are skipped).

Smoke path: `ai-nsc check|emit-ir|emit-obj|build|run` with `--target`,
`--opt`, `--dump-tokens/--dump-ast`, single or multiple input files.

Conformance defects that were reproduced are tracked in `docs/AUDIT.md`
(60 entries, all fixed/partial with the commit that closed them). A ✅ below
means the area has tests, not that every conceivable defect is ruled out.

## Matrix

| Spec area | Status | Notes / evidence |
|---|---|---|
| §2.1 source text files: UTF-8, `.ns` extension, NovaSharp-only content | ✅ | UTF-8 reads and the `.ns` extension are enforced by the driver; content is validated by the lexer/parser |
| §2 lexical: tokens, comments, `@` forced keywords, verbatim strings, encoding tags, contextual keywords, ranges | ✅ | lexer unit tests plus parser-side keyword shadowing (§2.4/§6.4): the parser tracks declared names per lexical block/member/type/module/function scope; a same-spelling declaration shadows the keyword in that and inner scopes, bare keywords in keyword position are then treated as names (and rejected where a name is not acceptable), and `@keyword` forces the keyword meaning (declarations, basic types and statement keywords). Enum/variant/union member and accessor/operator parameter scopes are not yet tracked. |
| Literal decoding: integer/float/character/string, encodings, raw `\x` bytes | ✅ | `src/literal.rs` unit tests |
| §3.1–3.2 type constructors, basic types, no implicit basic conversions | ✅ | e2e arithmetic, casts, comparison tests |
| §3.3–3.5 value/reference semantics, `&` by-reference parameters, scoped storage | ✅ | by-ref parameter and alias e2e tests |
| §3.6.1 arrays: fixed layout, indexing bounds, literals/repeat/range, array↔pointer casts, array parameter expansion, **VLA** | ✅ | e2e arrays/bounds/VLA tests; runtime dimensions are supported at any position in a function body. Multi-dimensional VLAs are represented as nested `{pointer, length}` levels (`array(2, n) T` is an array of two VLA rows; `array(n, m) T` is a VLA of VLA rows), with codegen allocating each row's buffer and initialising nested VLA storage; VLA initializers and VLA constants/returns remain rejected |
| §3.6.2 pointers: member access, `p ± i`, `p - q`, casts, `&`/`*` expressions | ✅ | e2e pointer tests |
| §3.6.3 function types, `wildcard`, calling conventions | ✅ | function value/closures tests; C/SystemV/Win64 LLVM conventions, others diagnosed |
| §3.7.1 classes: layout, inheritance, vtable/RTTI, construction, destructors, statics, abstract/sealed/static class | ✅ | class e2e tests, constructor-chain checks; a class/interface-typed field always occupies a reference (pointer) even when the field refers to the class being laid out (self-referential `Next: Node`), so instance size and codegen field offsets agree; the instance header carries vtable, typeinfo and an allocator slot (null for stack/default/malloc instances), used to dispose explicit-allocator heap instances returned from another function |
| §3.7.2 structures: exact layout, `aligned(N)` packing | ✅ | layout e2e tests |
| §3.7.3 interfaces: contracts, dispatch, RTTI, accessors, generics, interface inheritance | ✅ | interface dispatch/RTTI and inheritance tests (flattened methods, transitive `is` checks) |
| §3.7.4 enumerations: members, underlying type, casts | ✅ | enumeration e2e tests |
| §3.7.5 variants: state layout, payload construction, `is`/match, equality | ✅ | variant e2e tests |
| §3.7.6 unions: overlap layout, member offsets via `aligned`, member construction | ✅ | union e2e tests |
| §3.7.7 base lists: one class + interfaces, generic bases | ✅ | generics/bases tests |
| §3.8.1 fields, initializers (type-checked in declaring scope), readonly, accessibility | ✅ | field/initializer/readonly/accessibility tests |
| §3.8.2 accessors: extension/interface accessors, free accessors at module/function scope, backing get/set, expression/block bodies, static accessor alias | ✅ | accessor e2e tests; free accessors at module/function scope (local backings by reference, explicit members); a backing field supplies the missing default get/set automatically, and function-literal member shorthands (`get = (this, index) T => ...`) are unwrapped so `this`/index/value bind to the generated method |
| §3.8.3 extensions: members, extension-scoped functions, generic extension targets | ✅ | extension e2e tests |
| §4.1–4.4 value/parenthesized/cast/access expressions | ✅ | parser + e2e |
| §4.4.3 index access (arrays, VLA, indexer operators) | ✅ | bounds check raises `OutOfRangeException` |
| §4.5 calls: overloads, defaults, named arguments, function values, closures, nested functions, function pointers | ✅ | call/closure e2e tests; literal-family exactness prefers an integer/cardinal overload for integer literals and a floating overload for float literals (removing spurious ambiguity); Draft 1.153 §8.5 overload signatures are name + parameter count/order + base types, so overloads differing only by `#`/`&`/names/defaults/return type/calling convention are rejected, while contracts still participate in type equality and mangling |
| §4.6–4.7 construction and member construction | ✅ | construction tests |
| §4.8 `is` expressions: classes, interfaces, variants, special floats | ✅ | RTTI and `is illegal/infinite` tests; an infinite-value literal used against `infinite`/`-infinite` receives the default floating type context (§2.7.10) |
| §4.9 operators: overloads, derived comparisons, indexer, conversion, `implicitable`, user `??` | ✅ | operator/implicit conversion tests |
| §4.9.6/4.9.7 `&`/`*` reference expressions | ✅ | pointer/reference tests |
| §4.9.26 `??` with `Optional`/`Result` and all continuations | ✅ | coalescing + user `operator ??` tests |
| §4.9.27 `?:` conditional expression | ✅ | conditional e2e |
| §5.2 inline assembly: `assembly [with clobber(...)] { text + @[expr:dir] }`, escapes, placeholders, target syntax | ✅ | lexer keyword/raw-block scanner, parser (Intel-style text preserved, `@@`/`@}`/`@@[` escapes, `@[expr]`/`:in`/`:out`, duplicate/invalid clobber diagnostics), sema (function-body only, no compile-time/module/type/constant positions, lvalue rules, scalar operand types), LLVM InlineAsm lowering (outputs/tied read-write inputs, memory/cc clobber or global barrier, Intel dialect on x86; e2e mov/add); `check` now code-generates and assembles assembly-containing programs to a temporary object so invalid target text is a static error |
| §5 statements: blocks, locals/fixed, set/compound, if, while/do, for, match, break/continue, return, trap/finally, throw, dispose, export/use | ✅ | statement e2e tests; `finally` executes on fall-through and normal `return` paths (rejected `return` inside `finally`); `do-while` conditions are checked in the body block's scope; `dispose` targets must be heap allocation units (scope-lifetime instances are rejected per §5.8); direct local field pointer dispose targets are supported, arbitrary pointer/container paths are diagnosed (deviation 12); the `local @` no-optimize marker is only accepted immediately after `local` (applying to all items), not per item; runtime statements/expressions are rejected at file/module top level (only declarations, `#{}`, `#if` and `#()` are allowed there) |
| §5.6 `use` imports (per scope, later imports first, multi-file) | ✅ | use tests |
| §5.9 `match`: patterns, guards, logical control flow (break/continue/labels) | ✅ | match e2e tests |
| §5.9.5 `for`: arrays, VLA, ranges, `&` element alias, `IIterator`/`IIterable` targets | ✅ | for e2e tests |
| §6 names/scopes: modules, shadowing, `owner` chains, imports | ✅ | scope/owner/use tests |
| §7 modules (including nested modules and modules inside type bodies) | ✅ | module e2e tests |
| §8 functions: parameters, defaults, by-ref, array parameter sugar, `#` compile-time constants, nested functions | ✅ | parameter/call tests; `#` on functions, methods and operators |
| §4.9 operators: overloading/dispatch, one/two parameters, no `this`, by-reference parameters | ✅ | user-defined operators dispatch via overload resolution; parameters may be by-reference (operands must be addressable; addresses are passed so mutations are visible; compound assignments support them); match-pattern and conversion/accessor contexts that cannot provide an address report an explicit diagnostic |
| §9 modifiers: accessibility, `scoped`, `readonly`, `implicitable`, `manual`, `abstract`/`sealed`/`virtual`/`override`, `aligned`, `static` | ✅ | all enforced; every modifier may appear at most once (Draft 1.153 §9.5, including duplicate `scoped`); `verbatim` was removed in favour of the `@` marker (see §14.3.1) |
| §4.3 type conversion `Expression as Type` | ✅ | only the `as` operator at the `is` precedence is accepted; the Draft 1.13 `Cast<T>(value)` syntax was removed from the grammar and is now rejected |
| §4.9.15/§4.9.16 circular shifts `<<<`/`>>>` (and `<<<=`/`>>>=`) | ✅ | integer/cardinal operands, count taken modulo the operand bit width, no sign extension; runtime lowering and constant folding; compound forms supported |
| §5.9.5 directional ranges `..<`, `..<=`, `..>`, `..>=`; inferred `for` binding types | ✅ | parser/lexer, default step from the operator direction, direction-specific loop conditions, explicit types inferred from array/iterator/range targets; array-literal range expansion honours direction; step/direction contradictions diagnosed |
| §11.7 lazy two-phase conditional evaluation | ✅ | enabled automatically when a top-level/module `#if` condition references a not-yet-produced **immutable** binding (`#{}` `fixed` constant, `#` constant parameter, generic/type/platform constant): unconditional `#{}` execute first in file/source order, deferred `#if` settle in dependency rounds and lock their selected branch once (declarations collected, branch `#{}` executed once); `local`/mutable forward dependencies are diagnosed; a round that settles nothing reports a no-progress static error (e.g. mutual dependencies); Draft 1.153 removed fixed-point/oscillation detection and candidate-error buffering, so none is implemented. Top-level `#()` statements are parsed and executed (compiler-API calls in the declaration pass, other expressions in the second pass); forward bindings are shared across the whole program, and deferred `#if` declarations are restored into their original module scope during settlement. |
| §11.2/§11.4 compile-time execution: top-level `#{}`/`#if`/`#()`, function-body `#{}` scope and end products | 🟡 | top-level blocks and compile-time functions interpret constants, assignments, if/while/do-while/for-range, `match` with value/comparison/wildcard patterns and guards, API calls and compile-time calls; `#{}` does not scope declarations and retained end products stay visible; function-body blocks keep their end products when read at run time and reject run-time writes; scalar compile-time `heap` allocation/assignment/`dispose` is interpreted (the unit is freed with compile-time execution and is not visible at run time); non-scalar compile-time heap objects, `throw`/`trap` and `is` match patterns remain unimplemented in compile-time interpretation |
| §11.9 compiler API: Type/Compiler/Environment/NOLAA, `#if` environment queries | ✅ | compile-time contexts (`#{}`, `#(...)`, `#if`) may call the API; runtime code must wrap Type/Compiler/Environment calls in `#(...)`/`#if` (NOLAA markers remain run-time statements); Draft 1.153 §11.9.3 replaces `Platform` with `Environment`: `Current`/`Host`/`Target` facts are frozen, `PlatformIs`/`ArchitectureIs` compare canonical names byte-exactly and case-sensitively, unknown names answer false, `Other` has no name, and `#if` conditions use direct calls. `Type.SizeOf` returns a 64-bit unsigned value (32-bit targets use the low 32 bits; i686 test verifies). The compiler defines no extension names (permitted by §11.9.3). Current follows the target/program environment; Host gives the compiler-host facts explicitly. Value queries (`Environment.Current.Platform`/`.Architecture`, `Environment.Kind`) fold to `PlatformKind`/`ArchitectureKind`/`EnvironmentKind` constants, and cross-target architecture objects are emitted for x86_64/aarch64/riscv64 |
| §14.3.1 no-optimize marker: `@(expr)`, `@{ block }`, `local @Name`, `fixed @Name`, verbatim bodies | ✅ | parsed and enforced (nested markers, `#` inside markers and constant folding of marked storages rejected; `optnone` for marked bodies; e2e tests) |
| §5.9.5 `for` without an iteration variable (range only) | ✅ | parser/sema/codegen; range type inferred from the first non-literal expression and all range expressions must share it (literal `1` adopts `x`'s type in `for 1..x`); e2e ascending/descending/labelled-break and mixed-type diagnostics |
| §3.6.3 function types: `wildcard`, calling conventions, defaults, type equality/conversion | ✅ | `wildcard` matches unchecked trailing arguments (fixed prefix validated; small ints/bools promoted, `float(4)`→`f64`, arrays/VLAs decay to pointers); variadic declarations emitted; explicit calling conventions accepted after parameters/wildcard, bare `CallingConvention` member names resolved, `CDeclaration` uses the LLVM default C convention, `SystemVAMD64`/`MicrosoftX64` map to their LLVM conventions (method `wildcard` signatures still share the fixed-prefix matcher) |
| §3.8.2 accessor `this` without `&` for reference targets | ✅ | reference/value target validation (missing/extra `&`, wrong `this` type), extension blocks require `this`, static/free accessors reject it; block-shorthand receivers work for classes and structures (e2e) |
| §8.2 `array(Name: Type) T` parameter expansion | ✅ | function declaration parameters (`Name[dims]: T`), function type constructors and function literal parameter lists; sema expands to a pointer parameter followed by the binding parameters (e2e); parameter/argument/generic-parameter lists reject trailing commas/semicolons because the §8.2/§3.6.3/§10.1 grammars do not allow them; Draft 1.153 §11.7 no longer requires default expressions to be compile-time constants, and the e2e test shows a default calling a runtime function twice (caller-side evaluation) |
| §10 generics / monomorphization | ✅ | types/functions/constraints/extension-for-generic/caching/mangling; methods that declare their own type parameters; array extension targets (methods/accessors/operators); class/structure index accessors are tested, and array-target index accessors are rejected per §4.9.2 (index accessors may only be declared in class/structure extension blocks). Draft 1.153 §10.2 restricts the monomorphization base to an identifier (possibly module-qualified); any other expression is rejected. Function types carry per-parameter `#`/`&` contracts, so contracts participate in type equality and in the canonical spelling used by mangling. §14.3.2 optimisation directives are permissions, not requirements. |
| §11.1–11.7 compile-time: `#(...)`, `#if`/`#else`, `#{}` blocks (incl. top-level and cross-file order), `#` constant parameters, compile-time boundary | ✅ | compile-time e2e tests; blocks are lowered as runtime equivalents of compile-time results (semantics-preserving approximation) |
| §11.8 compiler API: `Type.SizeOf<T>`, `Compiler.LogNote/LogWarning/LogError/Assert`, `Environment.Current/Host/Target`, `NOLAA.Abandon/Bind/AssumeNoDemandNext` | ✅ | compiler API tests (environment queries are compile-time folded and reject runtime access) |
| §12 memory model: locals/heap/dispose/destructors, `manual`, custom `IAllocator` + `DefaultAllocator`, NOLAA (double-dispose/early-dispose/never-disposed warnings, alias/store demand tracking, auto-insertion at block-end/return/break/continue safe points, Bind groups, SCC grouping) | ✅ | heap/dispose/allocation/NOLAA tests; heap storage can hold value types (including integer/array/union targets) and assignment writes through to the heap storage; mutually-needed units linked through class fields are merged into one analysis group (automatic SCC) and disposed together; NOLAA reports escaped units as never disposed unless they were disposed or abandoned; analysis remains conservative for indirect aliases/pointers |
| §13 ABI: name mangling (§13.2), layouts, calling conventions, type info/vtables/interface tables | ✅ | mangling unit test + IR smoke tests; Draft 1.153 §13.2 hashes the per-parameter contract marker (`#`, `&`, `#&`, empty) before the encoded type; cross-target conventions beyond host diagnosed |
| §14 startup: launcher, static/runtime initializers, exit code = Main return | ✅ | launcher IR/e2e exit-code tests |
| §15 Built-In Library: `Object`, `CallingConvention`, `PlatformKind`, `ArchitectureKind`, `EnvironmentKind`, exception hierarchy with `InnerException`/constructors, `Optional<T>`, `Result<TValue; TAlternative>`, `IIterator<T>`, `IIterable<T>`, `IAllocator`, `DefaultAllocator`, compile-time library | ✅ | builtin library e2e tests; `EnvironmentFacts`/`Environment` queries are folded at compile time; containers are not part of the spec's §15.2 |

## Known deviations / remaining backlog

These are deliberate, documented limitations rather than silent miscompiles:

(§14.3.2 volume/speed optimisations are stated as permissions in the draft; not
implementing them is conforming and they are therefore not listed as deviations.)

1. **Compile-time blocks**: `#{}` no longer opens a lexical scope: its
   declarations stay in the enclosing compile-time scope and are visible to
   later code. Compile-time-only blocks are dropped (their locals are marked
   dead for codegen) and side effects warn (#75). A later **runtime** read of a
   block-declared variable keeps the block emitted as the carrier of its end
   product, while a runtime **assignment** to such a variable is diagnosed per
   §11.4. `do-while` and value/comparison/wildcard `match` (with guards) are now
   interpreted both at top level and in compile-time functions. Compile-time
   scalar `heap` allocation/assignment/`dispose` is interpreted (freed with the
   compile-time execution, never visible at run time); non-scalar compile-time
   heap objects, `throw`/`trap` and `is` match patterns are still not
   interpreted; such blocks are lowered with a warning or an explicit
   unsupported diagnostic, so §11.4's full compile-time execution remains
   partially implemented.
5. **`@` no-optimize markers** (§14.3.1): marked expressions, blocks, `local
   @Name` and `fixed @Name` force the enclosing function to `optnone`, are
   rejected in compile-time contexts, and marked expressions materialise literal
   operands through memory so the IR builder cannot constant-fold the marked
   expression (`@(1 + 2)` keeps its `add`). The `@` adjacency rules (no
   whitespace before a keyword, `(`, `{` or declared name) are diagnosed, a
   `fixed @Name` must not use a `@` expression as its initial value, and a
   `local @Name` initializer may not perform compile-time evaluation (§14.3.1).
6. **Compile-time libraries for external `linkable` functions** (§5.5: "编译期
   所需的库必须由编译器命令参数提供"): on the host target the compiler process's
   own C library acts as the compile-time library, so calls such as `printf` in
   `#{}` execute at compile time through a cached JIT trampoline (side effects
   happen during compilation and never enter the run-time artifact). Cross-target
   compilation has no host compile-time library and reports an explicit
   diagnostic. Compile-time infinite loops are not capped: an intentional
   `#{ while true { ... } }` is genuinely nonterminating, as specified.
6. **Single-expression function-literal bodies**: §2.7.9's EBNF allows only
   `Block | VerbatimBlock`, but Draft 1.153's own function-type-constructor
   example uses `(value: integer(4)) => value`. ai-nsc accepts the single
   expression and treats it as an implicit return; tests otherwise use block
   bodies. This is a spec-example accommodation, not a preferred extension.
6. **NOLAA** is a conservative approximation: warnings (including Early-Dispose),
   block-end/return/break/continue auto-dispose, alias/store demand tracking,
   `NOLAA.Bind` groups, `NOLAA.AssumeNoDemandNext`, and automatic SCC grouping of
   mutually-needed heap units linked through class fields. Direct aliases of heap
   units and pointer aliases (`local p = &heapUnit`) are tracked, so
   Double-Dispose and Use-After-Dispose are detected through
   `local alias = heapUnit;`, pointer members, and later assignment; aliases that
   could outlive the unit's block mark it escaped; pointer-member field writes and
   local container element writes are tracked as aliases (with escape propagation
   and use-after-dispose through the container). Dispose state is merged
   path-sensitively across if/match branches and loops (definitely vs maybe
   disposed), so a branch-only disposal no longer suppresses Never-Disposed and
   exclusive branches do not produce false Double-Dispose warnings. General CFG
   safe-point insertion, flow-sensitive points-to for arbitrary pointer/container
   element aliases and interprocedural summaries remain conservative (see
   `docs/NOLAA.md`).
8. **Target selection**: `--target` configures code generation, object emission
   and the semantic data layout for x86_64, aarch64, riscv64, i686 (runs natively
   on x86_64 hosts) and armv7 (cross-link with `--sysroot` + LLVM lld, run under
   qemu-arm). Other architectures are diagnosed; riscv32 output is object-only
   because no 32-bit RISC-V sysroot is available on the development host.
   **UEFI**: `--target=x86_64-unknown-uefi` builds a freestanding EFI application
   (§1's post-ExitBootServices bare-metal goal): the driver emits a COFF object via
   the Windows MSVC backend (LLVM 18 rejects `-uefi` triples on this host),
   generates an `efi_main` thunk that calls the generated `main`, and links with
   `lld-link /subsystem:efi_application /nodefaultlib`. There is no host C library
   and therefore **no platform `DefaultAllocator`** on UEFI: the developer supplies
   an `IAllocator` and uses explicit `heap` allocator expressions (allocators are
   the developer's responsibility); bare `heap` is diagnosed. `run` is rejected
   (boot with qemu + OVMF or real firmware); aarch64 UEFI is not supported yet.

12. **Pointer-member dispose targets**: §5.9's grammar allows
    `dispose ( Identifier | PointerMemberAccessExpression )`. A direct local
    field alias (`holder.Target = &heapUnit; dispose holder->Target`) is now
    tracked and disposed; arbitrary pointer/container/field paths still require
    full points-to analysis and are diagnosed rather than guessed.

13. **`EnvironmentFacts`/`Environment` are not materialised as ordinary
    built-in declarations**: §11.9.3's queries and the `PlatformKind`/
    `ArchitectureKind`/`EnvironmentKind` enumerations are implemented and
    compile-time folded, but the `EnvironmentFacts` structure and `Environment`
    module are recognised syntactically rather than declared in the built-in
    type space. This affects reflection over those built-ins only; all specified
    query forms are evaluated. Tracked for the final §15.2 audit.

14. **No-clobber assembly barrier breadth**: `assembly` without `clobber(...)` is
    emitted as `sideeffect` with `~{memory}` and `~{cc}` (a global memory/flag
    barrier). §5.2 also says all registers must be assumed clobbered; LLVM does
    not expose a portable "clobber all registers" constraint list, so register
    contents are not conservatively declared clobbered. Programs relying on the
    strongest barrier semantics should write `clobber(...)` explicitly.
