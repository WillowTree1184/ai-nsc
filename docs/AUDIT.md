# Spec conformance audit — defect register

Working list of reproduced conformance defects against
`docs/grammar/Draft 1.12.md`. The audit was a read-only review of the compiler at
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
