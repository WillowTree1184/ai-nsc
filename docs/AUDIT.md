# Spec conformance audit — defect register

Working list of reproduced conformance defects against
`docs/grammar/Draft 1.152.md`. The audit was a read-only review of the compiler at
commit `51bd259` (the commit before generics landed): each item was found by reading
the implementation against the specification and then reproducing it by compiling
small `.ns` programs. Rows marked `fixed (a5cc9cf)` were re-checked against their
reproduction and have a regression test in `tests/e2e_exec.rs` / `tests/cli.rs`;
the remaining rows are still open. A fix in flight in the working tree must still
be checked against the reproduction before its row is closed.

**How to reproduce.** Build once with
`CARGO_HOME=/tmp/ainsc-cargo LLVM_SYS_181_PREFIX=/usr/lib/llvm-18 cargo build`,
then compile the snippet with `target/debug/ai-nsc check prog.ns` (diagnostics only)
or `target/debug/ai-nsc run prog.ns` (execute; the exit code is `Main`'s return
value). A panic is a compiler bug, not a diagnostic.

Status values in the register: `open` (reproduced, no committed fix) — use `fixed`
only after the reproduction is re-run and its regression test is green.

| # | Severity | Spec § | Defect | Status |
|---|---|---|---|---|
| 1 | HIGH | §3.7.4 | Enumeration member used as a value panics the compiler (`type_defs` indexed by `TypeId`) | fixed (a5cc9cf) |
| 2 | HIGH | §3.7.4 | Integer↔enumeration explicit casts unsupported ("unsupported cast") | fixed (a5cc9cf) |
| 3 | HIGH | §3.6.1 | Array→pointer cast points at a temporary copy; writes are lost | fixed (a5cc9cf) |
| 4 | HIGH | §3.6.1 | Pointer→fixed-array cast copies; writes never reach the pointee | fixed (a5cc9cf) |
| 5 | HIGH | §5.9.1, §5.9.6 | `match` is not a labelled control flow in codegen; `break`/`continue`/named labels misbehave | fixed (a5cc9cf) |
| 6 | HIGH | §5.9.6 | `match` value/comparison patterns bypass user-defined `==`/`<`/`>` overloads | fixed (a5cc9cf) |
| 7 | HIGH | §3.7.1 | Base-class constructor is never called during derived construction | fixed (a5cc9cf + 8c0b507: `base.Construct(...)` verified end-to-end; missing-constructor and first-statement diagnostics added) |
| 8 | HIGH | ABI §13.2 | Mangled symbols embed internal type ids (`named(<id>)`), so symbols depend on declaration order | fixed (a5cc9cf) |
| 9 | HIGH | §5.10.4 | An unmatched inner `catch` does not reach the enclosing `trap`; the program aborts | fixed (a5cc9cf) |
| 10 | HIGH | §3.2 | Float→integer explicit cast emits bare `fptosi`/`fptoui` (UB out of range; spec requires wrapping) | fixed (a5cc9cf) |
| 11 | MED/HIGH | §3.6.2 | Pointer arithmetic missing (`p + i`, `p - i`, `p - q`); no pointer builtin overloads, no `Type.SizeOf<T>()` | partial (eafa0da `p ± i`, `i + p`, `p - q`; 2c5435c `p += i`/`p -= i`; `Type.SizeOf<T>()` still missing) |
| 12 | MED | §2.7.7 | `\x hh` escape produces a UTF-8 encoded scalar (2 bytes) instead of one raw byte | fixed (a5cc9cf) |
| 13 | MED | §6.4 | `owner` / `owner.owner` unsupported ("owner expressions are not implemented yet") | fixed (5fe9f0c: reads and assignment targets, nested chains, function-body scope merged) |
| 14 | MED | §5.6 | `use` statement is a no-op/error; imported members are not visible | fixed (0ec3a18: module members imported per scope; module-level `use` applies to later declarations) |
| 15 | MED | §8.2 | `&` parameters other than `this` unsupported (spec example `Scale(value: &integer(4), …)` rejected) | fixed (cea98d8: functions and methods accept further `&` parameters, aliasing call sites) |
| 16 | MED | §3.6.1 | VLAs (dimensions from runtime values) unsupported; dimensions are forced to constants | open |
| 17 | MED | §3.6.1 | `array(3) array(4) T` and `array(3,4) T` are distinct interned types (no flattening); fixed-array↔fixed-array casts rejected | fixed (9c701f0 flattening; c2217b6 explicit casts between equal-size array shapes, different byte sizes diagnosed) |
| 18 | MED | §6.3, §8.5, §14.1 | Duplicate declarations mostly undiagnosed (fields, enum members, variants, unions; signatures only at call sites; duplicate `Main`) | fixed (a5cc9cf) |
| 19 | MED | §9.10, §9.11 | Abstract class instantiation accepted; sealed-base check incomplete for classes without fields | fixed (a5cc9cf) |
| 20 | MED | §5.4 | Heap: type inference from the initializer missing; allocator expression ignored (always `malloc`) | partial (151ab56 type inference implemented; ecc4940 the allocator form is now an explicit 'not implemented yet' diagnostic instead of being silently ignored) |
| 21 | MED | §5.8 | `dispose` only accepts heap-allocated variables; spec example `dispose made;` (on a local) rejected; pointer-member dispose unsupported | fixed (scope-lifetime values dispose destructor-only; pointer-member dispose still unsupported) (97f6ec6) |
| 22 | MED | §5.10.2, §5.10.4 | Missing-return branch not diagnosed; `finally` containing `return`/`break`/`continue` not diagnosed | fixed (bf2416a + bacf958: `finally` control-transfer rejection and a conservative all-paths return judgement; loops are not proven to exit, so a trailing return may be required) |
| 23 | MED | §2.7.11, §2.7.12, §5.3 | `illegal`/`uninitialized` misuse undiagnosed; `fixed Bad: integer(4);` (no initializer) accepted | fixed (72587df + bf80e9b: constant initialization, illegal comparisons, and the `uninitialized` placement rule) |
| 24 | MED | §3.7.5 | Variant name-elision sugar missing (`local s: Status = Idle;`, `= Loading(50);`); built-in variant equality missing | fixed (9ef2245 payload-less elision, 40d12c7 payload elision, af2656d built-in variant equality) |
| 25 | MED | §3.6.1, §3.8.1, §5.8, §5.9, §12.2 | Warning subsystem unused: zero `Severity::Warning` sites; all spec warnings missing (index out of bounds, array↔pointer cast, match exhaustiveness, dispose warnings, initializer-order adjustment) | partial (ccb385b out-of-bounds index, 3bc3b49 array/pointer conversion, 8f6c41b variant match exhaustiveness, de212c5 double dispose; initializer-order adjustment is open) |
| 26 | LOW | §8.2 | Array parameter expansion sugar (`message[length: cardinal(4)]`) missing | partial (bf40b34: declaration expands to pointer+length parameters; implicit call-site conversion still missing) |
| 27 | LOW | §5.9.1 | `for &ref` element references missing | fixed (24244ab: by-reference array iteration implemented) |
| 28 | LOW | §5.7 | Compound-assignment evaluation order not specified/implemented | fixed (52650b1) |
| 29 | LOW | §3.2 | Signed `INT_MIN / -1` sdiv poison possible | fixed (52650b1) |
| 30 | LOW | §4.8 | `is` on statically impossible operands should evaluate to false rather than report an error | fixed (52650b1) |
| 31 | LOW | §4.8 | `is`-patterns over interface targets missing | fixed (52650b1) |
| 32 | MED | §8.2 | `#` compile-time parameters silently treated as runtime parameters | fixed (de34ae0: explicit 'not implemented yet' diagnostic in functions/methods/operators) |

## HIGH

### 1. Enumeration member as a value panics the compiler

```novasharp
enumeration Status: integer(4) { Running = 0; }
fixed Main(): integer(4) { local s: Status = Status.Running; return 0; }
```

Compiling this panics with `index out of bounds` (the audit reported
`src/codegen.rs:2807`). Cause: the `Resolution::EnumMember` arm of
`gen_value_from_resolution` indexes `self.sema.type_defs[type_id.0]` with the
`TypeId` instead of the `Type::Named(index)` index it encodes. `COVERAGE.md` marks
§3.7.4 ✅, but there are no enumeration execution tests.

### 2. Integer↔enumeration explicit casts unsupported

`Cast<Status>(Cast<integer(4)>(x))` fails with a codegen "unsupported cast" error;
`convert()` has no `(Integer, Named)` or `(Named, Integer)` arms. The spec allows
enumeration↔underlying-type explicit conversion (§3.7.4).

### 3. Array→pointer cast points at a temporary copy

```novasharp
local arr: array(4) integer(4);
local p: pointer integer(4) = Cast<pointer integer(4)>(arr);
p[0] = 42; // arr[0] is unchanged
```

`convert()` allocas and stores a copy, so the pointer does not alias the array.
The spec's point of an array→pointer conversion is aliasing (§3.6.1).

### 4. Pointer→fixed-array cast copies

The conversion does `build_load(array_type, ptr)`, materialising the pointed-to
array as a value; writes to the converted array never reach the pointee. Same
root cause family as #3, opposite direction.

### 5. `match` is not a labelled control flow in codegen

`gen_match_statement` never pushes `self.loops`, while sema does push the match
label (§5.9.1/§5.9.6). Consequences:

- `continue`/`break` inside a case target the enclosing loop instead of the match;
- named match labels fail in codegen;
- a `continue` targeting a top-level match passes `check` and then fails codegen.

### 6. Match patterns bypass user-defined comparison operators

Value/comparison patterns in `match` never resolve a user-defined `==`/`<`/`>`
overload in sema; codegen compares the raw values. Class operands fail with
"comparison is not defined" instead of dispatching to the operator (§5.9.6,
§4.9).

### 7. Base-class constructor is never called

`gen_construct_into` only calls the most-derived `Construct` method, so a derived
object never runs its base constructor; base fields set by the base `Construct`
stay zero. §3.7.1 requires construction to run the base constructor first.

### 8. Mangled symbols embed internal type ids

`TypeStore::write_spelling` emits `named(<id>)` for `Type::Named`, and
`Sema::spelling` only special-cases a top-level `Named`. Thus `pointer P` hashes
`pointernamed(9)` instead of `pointerP`; the same type spelled in a different
nesting position yields a different symbol, so symbols depend on
declaration order. The leaked id also appears in diagnostics. §13.2 / §7 require
stable, spelling-based mangling.

### 9. Unmatched inner `catch` does not reach the outer `trap`

When an inner `trap`/`catch` does not match a thrown exception type, dispatch must
fall through to the enclosing `trap` (§5.10.4). The current lowering abandons the
exception and the program aborts.

### 10. Float→integer explicit cast is UB out of range

`convert()` emits bare `fptosi`/`fptoui`. LLVM makes out-of-range results poison;
the spec requires the conversion to wrap (§3.2). Emit a range mask/clamping
sequence or a defined wrapper instead.

## MED/HIGH

### 11. Pointer arithmetic missing

`p + i`, `p - i`, `p - q` are rejected; there are no built-in pointer operator
overloads and no `Type.SizeOf<T>()` compile-time unit (§3.6.2). This blocks
low-level code and the spec's own pointer examples.

## MED

### 12. `\x hh` escape encodes a scalar instead of one byte

The escape produces the UTF-8 encoding of the scalar value (2 bytes for ≥ 0x80)
where §2.7.7 requires exactly one raw byte.

### 13. `owner` expressions unsupported

`owner` / `owner.owner` report "owner expressions are not implemented yet",
although §6.4 uses them to reach shadowed names in enclosing scopes.

### 14. `use` statement does not import members

`use` is a no-op or an error; members of the imported module are not visible
(§5.6). Multi-file input is also still pending (see `COVERAGE.md` §6).

### 15. `&` parameters other than `this` unsupported

The spec example `fixed Scale(value: &integer(4), factor: integer(4))` is
rejected; only method `this` may be by-reference (§8.2).

### 16. VLAs unsupported

Array dimensions that are runtime values are force-evaluated as constants
(§3.6.1). Fixed-dimension arrays work.

### 17. No array type flattening; fixed-array casts rejected

`array(3) array(4) T` and `array(3,4) T` intern as different types, with no
structural equality/flattening; explicit casts between fixed arrays are rejected
(§3.6.1).

### 18. Duplicate declarations mostly undiagnosed

Duplicate fields, enumeration members, variant states and union members are
accepted; duplicate function signatures are detected only at call sites; a
duplicate `Main` is accepted. §6.3/§8.5/§14.1 require declaration-time
diagnostics.

### 19. Abstract/sealed restrictions incomplete

Instantiating an abstract class is accepted (§9.10); the sealed-base check is
incomplete for classes without fields (§9.11).

### 20. Heap allocation gaps

Type inference from the initializer is missing and the allocator expression is
ignored (always `malloc`) (§5.4).

### 21. `dispose` too narrow

`dispose` only accepts heap-allocated variables; the spec's own example
`dispose made;` on a local is rejected, and disposing through a pointer member is
unsupported (§5.8).

### 22. Missing-return and `finally` diagnostics

A branch without a `return` in a non-void function is not diagnosed; `finally`
containing `return`/`break`/`continue` is not diagnosed (§5.10.2/§5.10.4).

### 23. `illegal` / `uninitialized` / uninitialized `fixed`

Misuse of `illegal` and `uninitialized` is undiagnosed, and
`fixed Bad: integer(4);` (a constant with no initializer) is accepted
(§2.7.11, §2.7.12, §5.3).

### 24. Variant name elision and built-in equality

`local s: Status = Idle;` and `= Loading(50);` are not accepted (name-elision
sugar), and variants have no built-in equality (§3.7.5).

### 25. Warning subsystem unused

There are zero `Severity::Warning` sites. All specification-mandated warnings are
missing: necessarily-out-of-bounds index (§3.6.1 L3894), array↔pointer and
pointer↔array casts (§3.6.1/§3.6.2), match exhaustiveness (§5.9), dispose
warnings (§5.8/§12.2), initializer-order adjustment failure (§3.8.1), and the
NOLAA double/early/never-disposed warnings (§12.2). Warnings must not block code
generation (front matter).

## LOW

### 26. Array parameter expansion sugar

`message[length: cardinal(4)]` in a parameter list (expanded to
`message: array(length: cardinal(4)) cardinal(1)`) is not accepted.

### 27. `for &ref` element references

By-reference `for` element bindings are missing (§5.9.1).

### 28. Compound-assignment evaluation order

The evaluation order of the target and the right-hand side in compound assignment
is not specified by the implementation (§5.7); fix the order and test it.

### 29. Signed `INT_MIN / -1` sdiv poison

Signed division can produce LLVM `sdiv` poison for `INT_MIN / -1`; guard it.

### 30. `is` on statically impossible operands

`is` between types with no possible overlap should evaluate to `false`, not
report an error (§4.8).

### 31. `is`-patterns over interface targets

`match`/`is` patterns whose target is an interface are not supported (§4.8).
| 33 | MED | §15.3 | Compiler compile-time API (`LogNote`/`LogWarning`/`LogError`/`Assert`) missing | fixed (5d1a968: literal messages and constant assertions inside compile-time blocks; computed messages pending) |
| 34 | MED | §9.15 | Static members (fields/constants/methods) unsupported | fixed (29cc794 static fields/constants/methods, f7b25ed static accessors backed by static fields and the 'static class' modifier; static accessors with explicit member bodies remain unimplemented) |
| 35 | HIGH | §4.9.26, §15.2 | `??` operator and built-in `Optional`/`Result` missing | fixed (394bdd4 built-in Optional/Result and continuations; 7b2a944 user-defined 'operator ??' decision operators) |
| 36 | HIGH | §2.7.9, §4.5 | Function values, function literals and indirect calls missing | fixed (648dbc3 named-function values/literals/indirect calls; 242b0ad capturing closures: closure objects with code+environment, by-value capture, escaping and nested closures) |
| 37 | HIGH | §4.5, §8.2 | Named arguments and default parameter values missing | fixed (12e923c: position/name mapping, defaults, diagnostics, works for functions/methods/static methods/constructors) |
| 38 | MED | §9.7, §4.3 | `implicitable` conversion operators were never invoked implicitly | fixed (dc370e6: implicit invocation in assignments/initializers/arguments/returns; explicit cast still required for non-implicitable operators) |
| 39 | MED | §15.2 | Built-in exceptions lacked `InnerException` and constructors | fixed (fb5529f: inherited `InnerException: Optional<Exception>`, Null default, `Exception(innerException:)` constructor, statics initialized) |
| 40 | MED | §4.9, §15.3 | Reference expressions (`&`/`*`) and NOLAA module missing; Platform.Is used non-canonical names | fixed (48334e9: `&`/`*` lowering including dereference assignment, NOLAA no-op hints, canonical case-sensitive platform names with false for unknown names) |
| 41 | HIGH | §11.5 | Compile-time ('#') constant parameters reported as unimplemented | fixed (4e8e97d free functions; a7b03bb methods; operator '#' parameters still reported as not implemented) |
| 42 | MED | §11.2 | Multiple input files rejected by the driver | fixed (3e2bcb1: files parsed into one AST in order, all actions accept several inputs, per-file diagnostics) |
| 43 | MED | §12.2 | Never-Disposed warning missing | fixed (e18b2cf never-disposed warning; d83c23e early-dispose warning; ec74e6b automatic dispose insertion at block safe points, conservative for escaping/no-safe-point units) |
| 44 | HIGH | §5.4, §15.2 | Custom heap allocators (`heap(allocator)`) and the IAllocator interface missing | partial (11d5fb8: IAllocator interface with Allocate/Deallocate, allocator expression checking, allocate/deallocate through the interface with AllocateFailedException; IIterator/IIterable and an end-to-end allocator test remain) |
| 45 | MED | §15.2 | `IIterator<T>`, `IIterable<T>` and `DefaultAllocator` absent from the Built-In Library | partial (8ee6895: IIterator<T>/IIterable<T> instantiated and usable, end-to-end test; DefaultAllocator and containers remain) |
| 46 | HIGH | §5.6 | `for` over `IIterator`/`IIterable` targets missing | fixed (ea017f6: iterator/iterable traversal with Next()/Value/Null, accessor conversion, break/continue) |
| 47 | MED | §15.2 | Built-In Library containers (array/list style iterables) absent | open |
| 48 | MED | §9.1-§9.4 | Accessibility modifiers not enforced on member access | partial (bb38cab: fields, methods, constructors, accessors and static fields enforce public/private/protected; function-value access rights (§9.1) remain open) |
| 49 | MED | §9.6 | `readonly` field assignment not enforced | fixed (d3942bf: assignment limited to constructors of the declaring/derived types) |
| 50 | MED | §3.8.1 | Instance field initializers were not type-checked | fixed (d5947c6: resolve in the declaring type's scope with implicit conversions and diagnostics) |
| 51 | HIGH | §3.6.1 | Variable-length arrays rejected | fixed (18cb471 VLA basics; bb97379 leading-runtime multi-dimensional VLAs, non-leading runtime dimensions diagnosed) |
| 52 | MED | §3.6.3 | Calling conventions ignored (function types always default) | fixed (fa6d5d8: convention in function types/declarations/methods/literals, LLVM CC on definitions and call sites for C/SystemV/Win64, unsupported conventions diagnosed) |
| 53 | MED | §8.4 | Methods required a first by-reference this; extension functions without this rejected | fixed (b844817: this of target type detected anywhere, class targets without '&', extension-scoped functions registered; value-type targets still require '&') |
| 54 | MED | §9.14 | `aligned(N)` validation and union offset semantics incomplete | fixed (01e1fa2: structure default alignment/packing, field overrides, union member offsets unadjusted, invalid targets diagnosed) |
| 55 | MED | §9.5 | `scoped(...)` accessibility clipping unimplemented | fixed (eaedf73 type/operator targets; 46d31aa member-name targets with override and sealed semantics) |
| 56 | MED | §5.3, §2.7.9 | Nested function definitions and '&function' pointers unsupported | fixed (4437802: nested functions as capturing literals bound to constants; function pointer address/deref calls) |
| 57 | LOW | §5.9.6 | `is illegal`/`is infinite` patterns in match unimplemented | fixed (59ebb2c: NaN and ±infinity patterns with float validation) |
| 58 | LOW | §5.9.5 | Range array literals without expected element type rejected | fixed (468f8ac: element type inferred from the range start or for binding type) |
| 59 | LOW | §all | Spec whole-program examples were only parse-tested | fixed (a59a0b4: 15+ sema-clean whole-program examples are compiled end to end by the CLI) |
| 60 | MED | §11.2, §11.4, §11.6 | Top-level compile-time blocks/variables and top-level #if unimplemented | fixed (2273414: in-order execution during declaration collection, cross-file variables, selected-branch declaration collection, runtime final values) |
| 61 | LOW | §9.8 | `verbatim` had no semantics | fixed (9e66017: optnone functions, runtime-evaluated verbatim storages, compile-time evaluation diagnosed inside verbatim) |
| 62 | MED | §10.1, §10.2 | Generic methods declaring their own type parameters unsupported | fixed (cf36878: method templates instantiated by `receiver.Method<T>(...)`, receiver compatibility, mangling/caching; missing type arguments diagnosed) |
| 63 | LOW | §9.5 | `scoped(operator Symbol)`/conversion targets not distinguished | fixed (5799c6c: symbol/conversion matching, parser support) |
| 64 | LOW | §11.4 | Compile-time expressions were lowered at run time | fixed (3b66c4d: recorded integer values emitted as folded constants; non-integer expressions fall back) |
| 65 | MED | §3.8.3 | Array extension targets unsupported | fixed (94c2f6d: array(length) T templates instantiated per array type with bound length constant; accessors/operators diagnosed) |
| 66 | MED | §3.8.2, §3.8.3 | Array extension accessors unsupported | fixed (fcc9f48: per-owner accessors, spec function-literal get form, bound length constant, per-instance AST clone) |
| 67 | LOW | §3.8.3 | Array extension operators unsupported | fixed (db74b74: per-owner operator registration, dispatch for non-named owners, lazy instantiation for operands) |
| 68 | MED | §12.2 | NOLAA had no return safe point (trailing-return heap units leaked) | fixed (2f41b11: dispose still-alive units before return, warning suppression updated) |
| 69 | MED | §12.2 | NOLAA had no break/continue safe points (loop-body heap units leaked) | fixed (73255c9: block/loop tracking, dispose loop-body units before break/continue) |
| 70 | MED | §12.2 | `NOLAA.Bind` groups, escape propagation and trackable-argument checking unimplemented | fixed (fdb1c9b: Bind merges analysis groups, group-wide safe points/escapes, strict argument diagnostics, compile-time APIs not escapes) |
| 71 | LOW | §11.4 | Compile-time-only `#{}` blocks emitted run-time code | fixed (db51aa2: marked compile-time-only blocks, locals pruned, code generation skipped) |
| 72 | LOW | §12.2 | `NOLAA.AssumeNoDemandNext` was a no-op | fixed (ebac2c9: next-statement demand suppression, one-statement scope) |
| 73 | MED | §13, §14 | `--target` only reached clang; LLVM module had no triple/data layout | fixed (d1d6d9b: x86/aarch64 target machine + triple/layout, cross object emission, cross-linking diagnosed) |
| 74 | LOW | §13, §14 | Only x86/aarch64 target backends wired | fixed (2bd5ce1: x86_64/aarch64/riscv64 cross code generation and objects; 32-bit targets diagnosed) |
| 75 | LOW | §11.4 | Compile-time block side effects emitted without warning | fixed (1b5b962: side-effect detection walk and specification-mandated warning; runtime-equivalent lowering documented) |
| 76 | LOW | §3.8.3 | Derived comparisons/indexer behaviour for array extension operators unverified | fixed (d7e5c71: derived `!=` verified via array extension `==`; indexer operators diagnosed) |
| 77 | HIGH | §12.2 | Heap aliases/stores were not NOLAA demands (possible use-after-free) and Early-Dispose warning was missing | fixed (6ddb3ee: alias/store demand tracking, early-dispose warning, regression tests) |
| 78 | LOW | §3.8.3 | Array target bindings accepted non-cardinal types | fixed (ba3d59c: cardinal-only validation) |
| 79 | MED | §5.5 | Bodyless `linkable` external symbols unsupported; linkable symbol literals kept their quotes | fixed (2478833: external function declarations, decoded symbol names, libc `abs` e2e) |
| 80 | MED | §3.7.3 | Interface inheritance (interface extends interface) unsupported | fixed (9e12cb9: base interfaces, flattened method lists, transitive class expansion, deferred implementation verification, receiver conversions) |
| 81 | MED | §9.15 | Static function-valued constants rejected ('static method requires a body') | fixed (e1815d6: expression-initialized static function constants as values, callable and passable) |
| 82 | MED | §3.8.2 | Free accessors (outside type bodies/extensions) unsupported | fixed (cb9acc2: free accessor function constants, default backing get/set, explicit members, read-only constant backing, scope visibility) |
| 83 | LOW | §3.8.2 | Function-scope free accessors with local backing unsupported | fixed (f05fe40: by-reference backing parameter getter/setter, read-only fixed backings; explicit members diagnosed) |
| 84 | MED | §12.2 | Linkable calls did not abandon NOLAA analysis / no Never-Disposed warning | fixed (47c9167: linkable abandonment marking, warning reporting, name-callee-only probe) |
| 85 | MED | §3.8.2, §9.15 | Static accessors with explicit members unsupported | fixed (d4e8e85: parameterless static getter/setter functions, default backing bodies, static member access/codegen) |
| 86 | LOW | §3.8.2 | Explicit members on local-backed free accessors diagnosed | fixed (5c4d942: member bodies bound to by-reference backing parameter, value parameter naming) |
| 87 | MED | §11.7 | Compile-time code could read regular runtime locals silently | fixed (47799f9: boundary diagnostic for non-compile-time locals; block locals/constants/# parameters allowed) |
| 88 | MED | §3.7.1, §7 | Modules inside type bodies unsupported | fixed (4bb904a: scoped identifier organization, nested modules, qualified access through the type name) |
| 89 | HIGH | §4.9.14 | Right shift used arithmetic shift for signed operands (runtime and constant folding) | fixed (a407fcb: logical zero-filling shift within operand width, both code generation and folding) |
| 90 | MED | §2.7.1, §3.2 | Minimum integer literal rejected; NaN float-to-integer cast produced poison | fixed (1e12fa6: minimum-literal typing under negation, deterministic NaN conversion) |
| 91 | MED | §4.9.24, §4.9.25 | Logical operators rejected implicitly boolean-convertible operands | fixed (59fcfea: implicitable conversion recorded and applied, short-circuit preserved) |
| 92 | MED | §4.9.27 | Conditional expression rejected valid base/derived branch orders | fixed (eff0f8c: symmetric common-type computation, context type precedence, branch coercions) |
| 93 | MED | §3.6.1 | Nested array spellings interned as distinct types | fixed (035ef15: array interning flattens nested dimensions; literals/declarations unify) |
| 94 | LOW | §5.3 | Global constants accepted 'uninitialized' as initial value | fixed (a364ce9: constant-initializer context tracked, specific diagnostic for all constants) |
| 95 | MED | §5.7 | Compound assignment ignored user-defined operators and accessor evaluation order | fixed (a6342ba: operator dispatch for compound assignments, RHS-before-current-value order) |
| 96 | MED | §11.4 | Top-level compile-time assignments were not executed | fixed (eca2869: compile-time assignment/compound assignment execution, final values as run-time constants) |
| 97 | MED | §11.4 | Compile-time blocks could not execute loops/branches | fixed (c95a4be: compile-time if/while execution, break/continue, constant conditions, iteration cap, inferred types) |
| 98 | MED | §11.4, §11.2 | Compile-time blocks could not call ordinary functions | fixed (51ad63d: post-resolution re-execution, integer function interpreter with recursion/loops, strict constant predicate preserved) |
| 99 | MED | §11.4 | Compile-time for-over-range loops unsupported | fixed (347f9a7: range for execution at top level and in interpreted functions, scoped binding, break/continue) |
| 100 | MED | §11.3 | '#(...)' rejected calls to ordinary compile-time functions | fixed (bb08e7d: compile-time-evaluable predicate, function interpretation, float-to-integer constant folding) |
| 101 | LOW | §13 | Layout sizing hardcoded 8-byte pointers (blocked 32-bit targets) | partial (a4d857b: TargetLayout plumbing for pointers/integer64/float64 alignment; default host unchanged, 32-bit triples still diagnosed) |
| 102 | MED | §11.4 | Compile-time execution was limited to the hand-written integer interpreter | partial (2100738: MCJIT evaluator for integer functions up to 64 bits, launcher-less module generation, JIT tests; semantic-analysis wiring pending) |
| 103 | MED | §11.4 | Compile-time execution did not use the JIT and rejected non-integer compile-time code | fixed (e9611cc: JIT-backed compile-time calls in phase B, live top-level constants, address-based JIT invocation, host-only with interpreter fallback) |
| 104 | MED | §13 | Code generation used hardcoded 8/16-byte pointer offsets | partial (9519b23: pointer_byte_offset(slots) for typeinfo/interface/exception/closure offsets; 32-bit still diagnosed because heap class layout is unverified) |
| 105 | HIGH | §12 | Heap-allocated structure read in an expression returns nondeterministic values | fixed (b166fc1: return expressions are evaluated before end-of-lifetime disposal; value spilled only when disposals exist; regression test) |
| 106 | HIGH | §13 | i686 interface dispatch used 8-byte vtable slot stride (segfault/typeinfo jump) | fixed (ae98012: slot addressing scales by pointer_byte_offset(1); i686 build/run enabled and tested with heap classes) |
| 107 | HIGH | §13 | armv7 build/link unsupported and 32-bit trap payload used an 8-byte offset | fixed (78d33eb: --sysroot + lld for ARM links, ARM32 layout, pointer-sized exception payload; qemu-arm heap-class and i686 trap tests) |
| 108 | HIGH | §3.7.1 | Heap-constructed class instances had no vtable/typeinfo header (interface dispatch crash) | fixed (7c3ab3f: emit_instance_header used by heap and stack construction; host/i686/armv7 verified, regression test) |
| 109 | HIGH | §14.3.1 | Draft 1.13 replaced `verbatim` with `@` no-optimize markers | fixed (parser/AST/sema/codegen for `@(expr)`, `@{ block }`, `local @Name`, `fixed @Name`, verbatim function bodies; nested/compile-time uses diagnosed) |
| 110 | MED | §5.9.5 | `for` did not accept an omitted iteration variable | fixed (range-only `for target` parsed/sema-checked/code-generated; e2e ascending/descending/labelled-break) |
| 111 | MED | §2.4/§2.6 | Keyword spellings as names were only partially contextual | partial (all name positions accept keyword spellings; declaration shadowing that disables a keyword meaning is still resolved as a keyword) |
| 112 | MED | §3.8.2 | Accessor block shorthand ignored the `this` reference/value rules | fixed (validated `this` type/number of `&` against the extension target; extension blocks require `this`; static/free accessors reject it; class and structure e2e) |
| 113 | MED | §3.6.1/§8.2 | `array(Name: Type) T` expansion unsupported in function type constructors and literal parameters | fixed (parser/sema expansion for both; call sites pass pointer+length, e2e) |
| 114 | MED | §11.2/§11.8 | Runtime code could call the compiler API directly | fixed (only `#{}`/`#(...)`/`#if` are compile-time contexts; Type/Compiler/Platform direct runtime calls diagnosed; NOLAA markers stay permitted) |
| 115 | MED | §11.8.1 | `Type.SizeOf` always returned cardinal(8) | fixed (value width follows the target pointer width: cardinal(4) on i686, verified by an i686 CLI test) |
| 116 | LOW | §2.1 | The compiler accepted source files with any extension | fixed (inputs must have the `.ns` extension; CLI test) |
| 117 | HIGH | §3.6.3 | `wildcard` parameters were rejected as "too many arguments" / "no overload" | fixed (FunctionInfo carries `wildcard`; the fixed parameter prefix is validated, trailing arguments are only type-checked for validity and passed unchecked with ABI promotions; variadic LLVM declarations/calls; user-defined and linkable printf cases tested) |
| 118 | HIGH | §5.9.5 | Omitted-binding `for 1..x` emitted invalid LLVM IR (i8 counter vs i32 bound) | fixed (with no explicit binding type the range type is inferred from the first non-literal expression and every range expression must share it; literal `1` adopts `x`'s integer(4); regression e2e + mixed-type diagnostics) |
| 119 | HIGH | §3.6.3 | `(wildcard; CDeclaration)` was a parse error | fixed (wildcard parameter list accepts a following `; CallingConvention`) |
| 120 | HIGH | §3.7.4/§3.6.3 | Bare `CDeclaration` in a calling-convention expression was an unknown name | fixed (calling-convention resolution accepts bare `CallingConvention` member names; full form still works) |
| 121 | HIGH | §3.6.3 | `CDeclaration` mapped to LLVM fastcc, so variadic printf/scanf declarations were rejected | fixed (`CDeclaration` uses LLVM's default C convention; variadic `...` declarations accepted; SysV/Win64 mappings unchanged) |
| 122 | MED | §2.7.1 | Integer literals inferred as `float(4)` (e.g. `Vector3(0,0,0)`) panicked in code generation | fixed (integer literals emit float constants when their inferred type is a floating-point type) |
| 123 | HIGH | §4.9/§8.3 | By-reference operator parameters were rejected even though the grammar allows `&` parameters | fixed (operators may take `&` parameters; dispatch requires addressable operands, address-based calls preserve mutation semantics, compound assignment supports by-reference operators; explicit diagnostics where an operand cannot be addressed) |
| 124 | MED | §5.5/§11.7 | A top-level compile-time block reported "unknown name" for linkable functions | fixed (phase-A blocks containing calls are deferred until declarations resolve; ordinary compile-time function calls execute as statements; external linkable calls at compile time report an explicit compile-time-library limitation instead of an unknown name; compile-time loops stop once an error is reported) |
| 125 | MED | §11.4 | Compile-time execution continued after an error and re-ran blocks, repeating diagnostics | fixed (a compile-time error aborts all further compile-time execution; phase-B re-execution is skipped once phase A reported a compile-time error; loop bodies stop on the first new error, so an intentional `#{ while true { ... } }` emits exactly one diagnostic) |
| 126 | LOW | §5.5 | The compile-time external-call diagnostic said "not implemented yet" instead of citing the missing compile-time library | fixed (message now: no compile-time library provides this external symbol; compile-time code may only call linkable functions supplied by a compile-time library) |
| 127 | MED | §5.5/§11.4 | External `linkable` calls could not execute at compile time and compile-time loops had an artificial iteration cap | fixed (host target: JIT trampoline executes external symbols such as printf at compile time via the host C library; fixed constant calls are cached; cross targets keep the compile-time-library diagnostic; iteration caps removed so an intentional `#{ while true { ... } }` is genuinely nonterminating per the specification) |
| 128 | MED | §1/§5.4 | UEFI was not a build target | fixed (x86_64 UEFI builds a freestanding PE32+ EFI application: Windows-MSVC COFF backend workaround, generated efi_main thunk, lld-link EFI subsystem, no host libc; platform DefaultAllocator is absent so allocators are the developer's responsibility; qemu+OVMF boot test) |
| 129 | HIGH | §4.3/§4.9/§5.9.5 | Draft 1.151 changed `Cast<T>(x)` to `x as T`, added circular shift operators and directional range operators, and allowed inferred `for` binding types | fixed (lexer/parser/AST/sema/codegen: `as` at the `is` precedence, `<<<`/`>>>`/`<<<=`/`>>>=` with width-modulo rotation, `..<`/`..<=`/`..>`/`..>=` with direction-specific defaults and conditions, inferred array/iterator/range binding types; legacy `Cast` was kept temporarily for compatibility and removed under #132) |
| 130 | HIGH | §11.2/§11.4/§11.7 | Draft 1.152 replaced the fixed-point/oscillation two-phase model of 1.151 with lazy two-phase evaluation (immutable forward bindings only, settlement rounds, no candidate buffering) | fixed (top-level/module `#if` deferral when a condition references a `fixed` constant produced by a later `#{}`; unconditional `#{}` startup execution; dependency-round settlement that locks each branch once and executes its `#{}`; `local`/mutable forward dependencies diagnosed; mutual dependencies report no-progress; boolean `fixed` constants in `#{}` supported; compile-time errors still terminate immediately) |
| 131 | MED | §11.4/§11.7 | Top-level `#()` statements were not parsed/executed and module-level lazy `#if` could not see the whole program's immutable bindings | fixed (top-level `#(expr)` parses as a compile-time expression statement and participates in the two-pass ordering; the future `fixed` set is collected from all files/modules before declaration collection, pending `#if` records its scope stack and settlement restores it so selected branch declarations land in the original module scope; cross-module forward bindings covered by tests) |
| 132 | HIGH | §2.6/§4.3 | The compiler still accepted the Draft 1.13 `Cast<T>(value)` syntax, which Draft 1.152 removed from the grammar (an extra, non-spec feature) | fixed (lexer keyword and parser production deleted; all tests/examples migrated to `Expression as Type`; new CLI regression asserts legacy `Cast<...>` is rejected) |
| 133 | HIGH | §2.4/§6.4 | Keyword shadowing was not implemented: a declaration spelling a keyword did not disable the keyword meaning, and bare shadowed keywords were still parsed as keywords | fixed (parser shadow-scope stack for blocks/member bodies/type/module/function bodies; declaration names registered for locals, fixed/heap/linkable, fields, types, modules, accessors and function/generic parameters; `peek_kind` downgrades shadowed keywords to identifiers, force flag supports `@keyword` for declarations, basic types and statement keywords; tests cover the spec's `class`/`integer` examples, `@if`, and bare-shadow rejection) |
| 134 | HIGH | §11.9.3 | `Platform.Is` answered the host platform even when compiling for another target | fixed (`--target` triple now selects the `PlatformKind` used by both `#if` and `#{}` evaluations, including `Other` for unnamed platforms; host remains the default; CLI regression covers host, Windows target and UEFI `Other`, and the named-vs-Other distinction) |
| 135 | MED | §11.4/§14.3.2 | Compile-time `do { ... } while (...)` blocks were not executed (lowered/unsupported) and §14.3.2 optimisations were incorrectly tracked as a conformance gap | fixed/clarified (top-level compile-time executor implements do-while with break/continue and constant-condition termination; §14.3.2 optimisations are draft permissions, so their absence is not a deviation) |
| 136 | MED | §4.9.2/§3.8.3 | Array-extension index accessors were reported as "not implemented"; class/structure index accessors were untested | fixed (index accessors in class/structure extensions verified end to end; an index accessor in an array-target extension is now an explicit §4.9.2 violation error instead of an "unsupported" placeholder; operator parameter names now open a shadow scope in the body, with a regression using a parameter literally named `class`) |
| 137 | HIGH | §11.4 | Function-body `#{}` blocks incorrectly opened a lexical scope (their declarations vanished afterwards), and end products were truncated even when later runtime code referenced them; runtime writes to compile-time block variables were accepted as ordinary locals | fixed (`#{}` now checks its statements in the enclosing scope; block-declared locals are tracked per block; a later runtime read keeps the block emitted as the end-product carrier and a runtime write is diagnosed; compile-time-only blocks mark their locals dead so codegen allocates no storage, preserving the no-runtime-code property) |
| 138 | MED | §14.3.1 | `@( ... )` no-optimize expressions were constant-folded by the IR builder and `@` accepted whitespace before keywords/`(`/names, contrary to the marker's rules | fixed (literals inside a marked expression are materialised through an alloca/store/load so arithmetic stays in the IR; the containing function becomes `optnone`, as do `local @Name`/`fixed @Name`; parser diagnoses whitespace between `@` and the keyword/`(`/`{`/name; IR regression asserts the `add i32` survives and `optnone` is present) |
| 139 | HIGH | §12.2 | Alias-aware NOLAA checks were missing: disposing a heap unit through a direct alias and then through the original (or vice versa) did not report Double-Dispose, and reading a unit after it was disposed through an alias did not report Use-After-Dispose/Early-Dispose | fixed (direct local aliases and later assignments map to their heap unit; dispose/read paths consult the unit's disposed flag, so Double-Dispose and Use-After-Dispose are reported through aliases; CLI regressions cover both) |
| 140 | HIGH | §2.7.1/§4.5/§4.8/§5.3/§5.4/§5.8/§5.9.4/§3.8.2 | Systematic audit of every `Main`-containing Draft-1.152 example exposed real gaps: literal expressions (`-2*3`, `1+2<<3`) did not adopt an expected integer type; `-infinite is -infinite` lacked float context; `Calculate(1,2)` was ambiguous against a float overload; heap value types could not be assigned/disposed; stack instances were accepted by `dispose`; accessors with only a backing were read-only and function-literal indexer members lost `this`; lambda-initialized function constants were unsupported; a `do-while` condition could not see body locals | fixed (all listed regressions added as e2e tests: literal inference 27, overload/infinite 3, heap value assign+dispose, accessor backing/indexer 9, do-while + lambda function constant 7) |
| 141 | CRITICAL | §3.7.1 | Self-referential class fields (`public Next: Node`) used the class stub's not-yet-finalised size (0) during layout, so the instance was under-allocated and codegen wrote fields past the heap block (memory corruption) | fixed (class/interface-typed fields always use the pointer size during layout; regression writes two self-referential heap fields and returns their sum, additionally verified under valgrind in development) |
| 142 | HIGH | §12.2 | NOLAA did not form automatic SCCs for mutually-needed heap units linked through class fields; such units were either treated as escapes or disposed independently | fixed (heap demand edges from tracked field stores; reachability check merges mutual units into one analysis group; regression with two heap nodes referencing each other disposes both destructors at the same safe point and runs cleanly) |
| 143 | MED | §11.4 | Compile-time `match` and `do-while` were not interpreted (top-level executor and compile-time function interpreter both skipped them) | fixed (top-level compile-time blocks and compile-time functions now execute `do-while` and `match` with value/comparison/wildcard patterns and guards; regressions return 3 for both a top-level match mutation and a compile-time function using do-while+match) |
| 144 | HIGH | §3.6.1 | Runtime array dimensions were only accepted in the leading position; non-leading runtime dimensions were diagnosed as unsupported | fixed (type resolution builds nested VLA levels at any dimension position; codegen allocates per-row buffers and recursively initialises nested VLA storage; tests cover `array(2, n)`, `array(n, m)` and nested `array(2) array(n)` forms with indexing; nested VLA initializers are rejected) |
| 145 | MED | §14.3.1 | `fixed @Name` accepted a `@` expression as its initial value and `local @Name` initializers could evaluate `#()` at compile time, contrary to the no-optimize marker rules | fixed (both cases diagnosed; regressions assert the specific errors; the marked-fixed initializer check also covers global constants) |
| 146 | MED | §5.2/§2.7.9 | `local a, @b` accepted a per-item no-optimize marker that §5.2's grammar does not allow (the `@` must follow `local` and applies to every item), and tests used expression-bodied function literals even though §2.7.9's EBNF requires a block | fixed for `local @` (only the leading marker is accepted; per-item marker is a parse error); test sources now use block-bodied lambdas, while single-expression bodies remain accepted only to parse Draft 1.152's own function-type example (documented as a spec inconsistency in `docs/spec-notes/_inconsistencies.md` and COVERAGE) |
| 147 | HIGH | §12.2 | The Never-Disposed check exempted every escaped heap unit, so a unit stored into a global/field and never destroyed produced no warning (the draft requires the check for every non-manual unit unless abandoned) | fixed (escaped units now participate in the Never-Disposed warning; explicit dispose/abandon/manual and jump-dispose units are still exempt; regression stores a heap unit into a global and asserts the warning) |
| 148 | HIGH | §11.8/§5.4 | Compile-time `#{}` blocks could not execute scalar `heap` definitions/assignments/`dispose`, so §11.4's "local and heap variables execute as normal statements" was unmet | fixed (scalar compile-time heap units are tracked separately from run-time constants, assignments/compound assignments write through, `dispose` frees them, and the unit never becomes visible to run-time name resolution; non-scalar compile-time heap objects are explicitly diagnosed; regressions cover assignment/dispose, a heap counter loop, runtime invisibility and the non-scalar diagnostic) |
| 149 | HIGH | §5 | Runtime statements and expressions (`if`/`while`/`for`/bare expressions) were silently accepted at file and module top level even though §5 only permits statements inside function bodies | fixed (declaration collection validates file/module top-level nodes: only declarations, `#{}`, `#if` and top-level `#()` expressions remain legal; everything else reports "statements and expressions are only allowed inside function bodies"; the empty top-level block in a regression was removed and two CLI tests cover file/module rejection) |
| 150 | MED | §5/§14 | The spec-conformance test silently skipped every `Main` example that failed semantic analysis, so new regressions could hide; two remaining failures (top-level loop example, returned-heap dispose) were undocumented | fixed the audit harness: a closed-set test now asserts the exact list of sema-failing spec programs, and both cases are documented in `docs/spec-notes/_inconsistencies.md` and COVERAGE (top-level loop is a draft inconsistency; returned-heap ownership is a remaining NOLAA gap) |
| 151 | HIGH | §5.8/§12.2 | Disposing a heap unit returned from another function (the §5.8 example with `Make()`/`dispose made`) was diagnosed as "dispose requires a heap allocation unit", and the return path also mis-ordered NOLAA escape marking so the callee could free the value it returned (double free) | fixed (syntactic pre-pass marks functions returning default-allocator heap locals; call results assigned to locals become tracked heap references; codegen destructs and deallocates through DefaultAllocator; returned units transfer ownership, so Never-Disposed no longer warns and return-escaping happens before return-dispose planning; regressions cover the full example end to end and the strict spec-failure set no longer lists line 6285) |
| 152 | MED | §8.2/§3.6.3/§4.5/§10.1 | The parser accepted trailing commas in parameter lists, function-literal parameter lists and call argument lists, and a trailing `;` in generic parameter lists, none of which the draft grammars permit (extra syntax) | fixed (explicit diagnostics for trailing comma in parameter/argument/literal-parameter lists and trailing `;` in generic parameter lists; regression covers function declaration and call cases) |
| 153 | MED | §5.4/§5.8 | Returning a heap class instance allocated with an explicit allocator and disposing it in the caller was rejected because the allocator was not available at the caller; cross-function explicit-allocator ownership was a documented gap | fixed for class types: the class instance header gains a third slot storing the explicit allocator (null for default/malloc/stack instances), heap allocation records it, and the caller's `dispose` loads the slot and dispatches `IAllocator.Deallocate`; structure/value-type explicit-allocator returns remain diagnosed (no instance header). Custom-allocator allocation sizes stay exactly the instance size, so the §5.4 size rule and existing allocator tests are preserved. |
| 154 | MED | §5.8 | The parser accepts a pointer-member dispose target, but the compiler rejected it with the generic heap-unit diagnostic without documenting that §5.8 explicitly allows `dispose PointerMemberAccessExpression`; field points-to analysis is absent | documented (COVERAGE deviation 12 and §5 statement row) and regression asserts the explicit diagnostic so the gap cannot be mistaken for a silent limitation; implementation of field points-to/alias analysis remains |
| 155 | HIGH | §5.9 | `dispose holder->Target` (a pointer-member dispose target allowed by §5.9) was always diagnosed even for the direct local-field alias case `holder.Target = &heapUnit`; no field alias was tracked | fixed (direct local member aliases are recorded when the member is assigned `&heapUnit`/a heap local; `dispose holder->Member` resolves to that unit, shares NOLAA dispose state and codegen disposal; the e2e regression disposes through the field and reads the value back; arbitrary pointer/container paths remain documented as deviation 12) |
| 156 | HIGH | §5.3/§5.10.6/§2.7.9 | Draft 1.153 switched the authoritative examples to block-bodied function literals, braced match cases and a function-wrapped `for` example, but the test harness still read 1.152 and the parser still accepted the 1.152 tolerances (expression bodies, `case pattern:` single statements) | fixed baseline (spec tests read `Draft 1.153.md`; line whitelists/fragments updated to 1.153; expression-body and colon-case tolerances removed; inline-assembly and type-constructor examples are temporarily treated as syntax fragments until §5.2/Environment implementation; parser-checker now follows 1.153) |
| 157 | HIGH | §8.5/§13.2 | The overload signature accepted two same-name functions whose parameter types were equal after stripping `#`/`&` (e.g. `F(value: integer(4))` and `F(#value: integer(4))`), and mangling hashed only the type, so such overloads were neither rejected nor distinguishable in symbol names | fixed (Draft 1.153 §8.5: signature conflicts compare parameter count/order/base types and ignore contracts/names/defaults/return/convention; global, method and free-function registration now reject duplicate signatures. §13.2 mangling prefixes each parameter's hash input with its `#`/`&` contract marker; regression covers both contract-only overload cases) |
| 158 | MED | §9.5 | Repeated modifiers (notably `scoped`) were silently accepted/last-wins; Draft 1.153 explicitly forbids repeating a modifier | fixed in the modifier parser (`a modifier must not repeat (§9.5)`); regression covers duplicate `scoped` |
| 159 | MED | §10.2 | The parser accepted monomorphization on arbitrary expressions, while Draft 1.153 restricts the base to an `Identifier` | fixed (only a name or module-qualified name chain is accepted as a monomorphization base; other expressions get a parse diagnostic; regression) |
| 160 | HIGH | §11.9.1 | `Type.SizeOf<T>()` returned a target-pointer-width integer type, contradicting Draft 1.153's fixed 64-bit unsigned value width (32-bit targets may use the low 32 bits) | fixed (result type is always `cardinal(8)`; i686 test updated to bind a 64-bit value and verify the size; 32-bit low-word use is covered by the target test) |
| 161 | HIGH | §11.9.3 | Draft 1.153 replaces the `Platform` API with `Environment`: `Current`/`Host`/`Target` facts, frozen platform/architecture facts, `PlatformIs`/`ArchitectureIs` string queries, `ArchitectureKind`/`EnvironmentKind`; the compiler still exposed `Platform.Is` (extra 1.152 API) and lacked host/target architecture facts | fixed (Platform handling and namespace removed; Environment queries are recognised and compile-time folded for Current/Host/Target, canonical names are byte-exact/case-sensitive with unknown names false, `Other` has no name, architecture facts added to Sema, runtime access is rejected unless inside `#()`/`#if`; built-in `ArchitectureKind`/`EnvironmentKind` enumerations added; tests migrated from Platform to Environment and new host/target/architecture tests added; `Platform` is now an unknown name) |
| 162 | HIGH | §5.2 | Inline assembly was entirely unimplemented (keyword absent, `assembly` examples failed to parse); Draft 1.153 makes it a normative statement with placeholders, directions, escapes, clobbers and target syntax | fixed (keyword + raw-block scanner preserving assembly text, `@@`/`@}`/`@@[` escapes, `@[expr]`/`:in`/`:out` placeholders, clobber parsing/duplicate checks; sema placement/lvalue/type rules; LLVM InlineAsm lowering with output/tied read-write operands, memory/cc clobber or global barrier, Intel dialect on x86. e2e `mov`/`add` returns 76; diagnostics cover inout, non-lvalue out, duplicate clobber, top-level/compile-time placement; escapes emit-IR test. Remaining limitations recorded as COVERAGE deviations 14/15: target syntax validation deferred to object emission, and no-clobber only models memory/cc.) |
| 163 | HIGH | §5.2 | Invalid inline-assembly text was only rejected during `build`/`emit-obj`; `check` reported success even though §5.2 requires target syntax to be validated statically | fixed (SemaResult records assembly presence; `check` code-generates assembly-containing programs and assembles them to a temporary object with the selected target, reporting LLVM's target error; regression checks an invalid mnemonic and a valid `add`). Remaining limitation: no-clobber register-barrier breadth (COVERAGE deviation 14) |
| 164 | MED | §5.2 | An unannotated `assembly` block only declared memory/condition-code clobbered; §5.2 also requires condition flags to be assumed clobbered | partially fixed (unannotated blocks now emit `~{memory},~{cc},~{flags},~{fpsr},~{dirflag}`; the full "all registers" part remains impossible with LLVM's register allocator because clobbering every GPR leaves no register for the asm operands, recorded as COVERAGE deviation 14; IR regression asserts the barrier clobbers) |
| 165 | HIGH | §3.6.3/§8.5 | Function types stored only parameter types, so `#`/`&` contracts did not participate in type equality (Draft 1.153 requires both), allowing plain functions to be assigned to contract-qualified function types | fixed (Type::Function now carries a per-parameter contract vector; constructors collect contracts from declarations/function values/literals; equality and interning include them; the canonical type spelling prefixes `#`/`&` markers so nested function types also affect mangling). Regression: assigning `function(integer(4))integer(4)` to `function(#integer(4))integer(4)` is rejected, same-contract assignment succeeds |
| 166 | HIGH | §12.2 | Pointer aliases (`local p = &heapUnit`) were treated as unconditional escapes: no auto-dispose at the unit's safe point, a spurious Never-Disposed warning, and no Use-After-Dispose diagnostic through `p->Field` | fixed (pointer locals map to their unit; same-block/nested aliases permit block-end disposal, aliases that can outlive the unit block mark it escaped, escaping a pointer alias propagates to the unit, and disposed state is visible through pointer member access; regressions cover safe disposal, use-after-dispose and outer-scope escape; remaining CFG/points-to work tracked in `docs/NOLAA.md`) |
| 167 | HIGH | §12.2 | Dispose state was a single global flag: disposing a unit in only one `if` branch suppressed Never-Disposed (leak on the other path), and disposing in both exclusive branches produced a false Double-Dispose; branch regions also inherited each other's state | fixed (definitely/maybe disposed state is snapshotted and merged over `if`/`else-if`/`else`, `match` cases with wildcard exhaustiveness, and `while`/`do-while`/`for`; jump/auto disposal skips units that may already be disposed, and Never-Disposed is reported when disposal is only on some paths. Regressions cover one-branch leak, both-branch no-warning, wildcard-match disposal) |
| 168 | HIGH | §12.2 | Pointer-member field writes (`p->Field = &unit`) and local container element writes (`bucket[i] = unit`) were treated as unconditional escapes: no tracked alias, spurious Never-Disposed, and no use-after-dispose through the container | fixed (field aliases accept pointer-member targets; local element writes record aliases tied to the container's block, propagate container escape to the units, allow block-end disposal when the container is local, and report use-after-dispose when a disposed unit is read through `bucket[i]`. Regressions cover pointer-member disposal, container safe disposal/use-after and direct aliases) |
| 169 | HIGH | §5.9/§12.2 | Disposing a heap unit through a function parameter was rejected ("dispose requires a heap allocation unit"), so helper functions could not take ownership and callers got spurious Never-Disposed warnings | fixed for class parameters (a top-level `dispose param` marks the function as disposing that parameter; codegen frees through the class-header allocator or DefaultAllocator; call sites mark the argument unit disposed and reject arguments that do not point to a heap unit). Regressions cover helper disposal without warnings, stack-argument rejection, caller double-dispose and a destructor-count e2e; summary analysis remains conservative for branching bodies and methods |
| 170 | HIGH | §12.2 | Automatic disposal was only emitted at block end, so a unit stayed alive across unrelated statements after its last use; the safe-point analysis did not select a last-demand statement | fixed (each heap unit records the latest demand statement; when all demands are direct statements of the unit's block, disposal is emitted immediately after the last one; nested uses, jump-disposal paths and always-return paths fall back to block end. Codegen gained per-statement disposal maps; e2e verifies destructor ordering: 1 then 2 => 12) |
| 171 | MED | §12.2 | A heap unit used only inside a nested control statement (if/loop) was classified as "nested" and forced back to block-end disposal, leaving it alive across unrelated following statements | fixed (demands record the outermost direct statement of the unit's block, so dispose is emitted after the enclosing control statement when no later demand exists; jump/always-return paths still fall back. E2e destructor ordering with a nested `if` returns 12) |
| 172 | MED | §5.2 | Operand-free unannotated assembly only declared memory/flag clobbers; §5.2 requires all registers to be assumed clobbered | partially fixed: operand-free blocks now list all target GPR/vector registers plus memory/flags; operand blocks cannot (LLVM allocator has no free registers) and still require explicit `clobber(...)`, recorded as COVERAGE deviation 14. IR regressions cover both cases |
| 173 | HIGH | §12.2 | Every call argument was unconditionally marked escaped, so passing a heap unit to a helper that only reads it caused a false Never-Disposed and prevented safe-point disposal | fixed (per-parameter escape summaries are recorded while checking callee bodies; a callee parameter that is not returned/stored/thrown/passed on is read-only, and call sites then keep the caller's unit disposable. Unknown/later-checked callees remain conservative; regression covers read-only helper disposal and a storing helper still escaping) |
| 174 | MED | §11.9.3/§15.2 | `EnvironmentFacts` was not a declared type and `Environment.Current`/`Host`/`Target` were not values, so a program could not store facts in a local or read `.Platform`/`.Architecture` fields | partially fixed: built-in `EnvironmentFacts` structure (Platform/Architecture) and frozen Current/Host/Target values are materialised; facts locals are tracked, field reads and PlatformIs/ArchitectureIs fold at compile time, and codegen emits the frozen struct for runtime storage. Remaining: the `Environment` module declaration and declared member functions for unknown facts values (COVERAGE deviation 13). E2e stores host facts and returns 7. |
| 175 | MED | §15.2/§11.9.3 | The built-in `Environment` module was not declared, so module-qualified members existed only through syntactic query interception; `Environment.Kind`/`Current`/`Host`/`Target` could not be resolved as module members | fixed (a built-in `Environment` module scope is created with linkable constants Kind/Current/Host/Target of the proper enum/facts types; expressions still fold to frozen constants; e2e reads `Environment.Kind` and `Environment.Target.Platform` and returns 3). Remaining deviation 13: member functions on facts values without a statically tracked origin |
| 176 | MED | §15.2/§11.9.3 | `EnvironmentFacts` declared `PlatformIs`/`ArchitectureIs` methods in §15.2, but the compiler type had no such members; calls on a facts value whose origin was not tracked produced an unclear missing-member error | fixed (the structure now declares both member functions; frozen-value calls fold as before, and calls on an untracked facts value report an explicit "environment facts known at compile time" diagnostic instead of a missing member). Regression covers the diagnostic; residual limitation recorded in COVERAGE deviation 13 |
| 177 | MED | audit | The COVERAGE deviation list mixed real semantic gaps with already-implemented features (no-optimize markers, compile-time external libraries) and platform capabilities; numbering was inconsistent and the value-type explicit-allocator return was not listed | fixed documentation audit: deviations renumbered to the six real remaining gaps (compile-time non-scalar/throw/trap/is, expression-bodied spec accommodation, NOLAA precision, value-type explicit-allocator returns, unknown-origin EnvironmentFacts queries, operand-block assembly barrier); implemented features and target capabilities moved to a separate non-deviation section |
| 178 | HIGH | §5.11.2/§12.2 | Returning a heap-allocated *value-type* instance (structure) was treated like a reference-return ownership transfer: the unit was marked escaped/returned, so the callee never disposed it (leak) and the caller received only a copied value | fixed (only class/interface reference-type units transfer ownership on return; value-type units stay demand-tracked and are disposed at the callee's return/jump safe point after the copy. E2e: heap structure returned by value, field read in caller, destructor ran once, result 15) |
| 179 | HIGH | §5.2 | An unannotated `assembly` block *with operands* could not declare all registers clobbered: LLVM rejected clobbering every GPR and the block only declared memory/flags, leaving register values unsound across the asm | fixed (operand blocks in unannotated assembly are lowered with each operand pinned to a fixed target register and every other target register declared clobbered; works on x86_64, i686, aarch64, armv7, riscv64 object emission, plus x86_64 runtime. COVERAGE deviation removed) |
| 180 | MED | §11.9.3 | A `PlatformIs`/`ArchitectureIs` query on a facts-typed parameter (origin not statically tracked) was rejected outright, so a conforming function taking `EnvironmentFacts` could not query it | partially fixed: runtime calls now lower to a comparison of the frozen facts field against the queried canonical kind (e2e `IsLinux(Environment.Host)` returns 9); compile-time `#(...)` queries still require statically known facts, so specialization-by-fact-value remains (COVERAGE deviation 4) |
| 181 | MED | audit | COVERAGE counted NOLAA precision limits as deviations even though §12.2 explicitly requires conservative non-insertion when a safe point cannot be asserted, and counted the §2.7.9 function-literal example accommodation as a deliberate extension | fixed audit classification: NOLAA conservatism moved to capabilities (it is the specified behavior), the expression-bodied function literal moved to a separate "specification inconsistencies" section (EBNF vs its own example), leaving two real deviations: compile-time non-scalar execution and EnvironmentFacts compile-time specialization |
| 182 | HIGH | §11.9.3 | Compile-time `#(facts.PlatformIs(...))` in a function taking an `EnvironmentFacts` parameter was rejected, even when every call site passes `Environment.Host`/`Current`/`Target`; the compiler did not specialize functions by facts value | fixed (functions with facts parameters are instantiated per call-site fact tuple, with the parameter local mapped to its frozen host flag and the body re-checked, so queries fold; forwarding a facts parameter through another facts function defers and specializes the caller too. Calls whose facts origin is not statically tracked still diagnose. E2e specialization chain returns 6) |
| 183 | MED | §11.9.3 | A call to a function returning a constant `EnvironmentFacts` value was not recognised as a facts source, so passing it to a facts-parameter function failed specialization even though the origin was statically known | fixed (return statements of facts-returning functions record a known host/current/target summary; single-target calls are treated as facts sources during call-site specialization and re-check the callee body. E2e `IsLinux(Facts())` returns 5; residual remains for facts loaded from fields) |
| 184 | MED | §11.9.3 | A facts value stored in a local object's field was not tracked, so passing that field to a facts-parameter function failed compile-time specialization even though the field was assigned `Environment.Host` in the same function | fixed (Set statements record known facts for `(local object, field)` pairs and reads of such fields are recognised as facts sources; e2e `holder.Facts = Environment.Host; IsLinux(holder.Facts)` returns 4. Residual: fields of parameters/unknown objects) |
| 185 | HIGH | §11.4 | Top-level compile-time `trap`/`throw`/`finally` was rejected as not implemented, so compile-time exception control flow could not execute | partially fixed: compile-time flow carries a thrown type name, `trap` executes wildcard/syntactic-typed catches and `finally`, uncaught throws are diagnosed, and end products remain visible (e2e wildcard/typed/finally returns 42; uncaught test). Remaining: thrown values/catch bindings, class-hierarchy catch matching, non-scalar heap and `is` patterns (COVERAGE deviation 1) |
