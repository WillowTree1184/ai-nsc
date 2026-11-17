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
| 11 | MED/HIGH | §3.6.2 | Pointer arithmetic missing (`p + i`, `p - i`, `p - q`); no pointer builtin overloads, no `Type.SizeOf<T>()` | partial (eafa0da: `p ± i`, `i + p`, `p - q` implemented with pointee-size unit; `Type.SizeOf<T>()` and `p += i` still missing) |
| 12 | MED | §2.7.7 | `\x hh` escape produces a UTF-8 encoded scalar (2 bytes) instead of one raw byte | fixed (a5cc9cf) |
| 13 | MED | §6.4 | `owner` / `owner.owner` unsupported ("owner expressions are not implemented yet") | fixed (5fe9f0c: reads and assignment targets, nested chains, function-body scope merged) |
| 14 | MED | §5.6 | `use` statement is a no-op/error; imported members are not visible | fixed (0ec3a18: module members imported per scope; module-level `use` applies to later declarations) |
| 15 | MED | §8.2 | `&` parameters other than `this` unsupported (spec example `Scale(value: &integer(4), …)` rejected) | open |
| 16 | MED | §3.6.1 | VLAs (dimensions from runtime values) unsupported; dimensions are forced to constants | open |
| 17 | MED | §3.6.1 | `array(3) array(4) T` and `array(3,4) T` are distinct interned types (no flattening); fixed-array↔fixed-array casts rejected | open |
| 18 | MED | §6.3, §8.5, §14.1 | Duplicate declarations mostly undiagnosed (fields, enum members, variants, unions; signatures only at call sites; duplicate `Main`) | fixed (a5cc9cf) |
| 19 | MED | §9.10, §9.11 | Abstract class instantiation accepted; sealed-base check incomplete for classes without fields | fixed (a5cc9cf) |
| 20 | MED | §5.4 | Heap: type inference from the initializer missing; allocator expression ignored (always `malloc`) | open |
| 21 | MED | §5.8 | `dispose` only accepts heap-allocated variables; spec example `dispose made;` (on a local) rejected; pointer-member dispose unsupported | fixed (scope-lifetime values dispose destructor-only; pointer-member dispose still unsupported) (97f6ec6) |
| 22 | MED | §5.10.2, §5.10.4 | Missing-return branch not diagnosed; `finally` containing `return`/`break`/`continue` not diagnosed | fixed (bf2416a + bacf958: `finally` control-transfer rejection and a conservative all-paths return judgement; loops are not proven to exit, so a trailing return may be required) |
| 23 | MED | §2.7.11, §2.7.12, §5.3 | `illegal`/`uninitialized` misuse undiagnosed; `fixed Bad: integer(4);` (no initializer) accepted | partial (72587df: constant-initializer and illegal-comparison diagnostics; `uninitialized` placement rule still open) |
| 24 | MED | §3.7.5 | Variant name-elision sugar missing (`local s: Status = Idle;`, `= Loading(50);`); built-in variant equality missing | open |
| 25 | MED | §3.6.1, §3.8.1, §5.8, §5.9, §12.2 | Warning subsystem unused: zero `Severity::Warning` sites; all spec warnings missing (index out of bounds, array↔pointer cast, match exhaustiveness, dispose warnings, initializer-order adjustment) | open |
| 26 | LOW | §8.2 | Array parameter expansion sugar (`message[length: cardinal(4)]`) missing | open |
| 27 | LOW | §5.9.1 | `for &ref` element references missing | open |
| 28 | LOW | §5.7 | Compound-assignment evaluation order not specified/implemented | open |
| 29 | LOW | §3.2 | Signed `INT_MIN / -1` sdiv poison possible | open |
| 30 | LOW | §4.8 | `is` on statically impossible operands should evaluate to false rather than report an error | open |
| 31 | LOW | §4.8 | `is`-patterns over interface targets missing | open |

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
