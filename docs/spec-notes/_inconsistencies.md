# Draft 1.153 inconsistencies observed while implementing

The specification is a moving draft. This file records the places where a
literal reading of the grammar contradicts the prose or the spec's own examples,
and what ai-nsc does about each one. Line numbers refer to
`docs/grammar/Draft 1.153.md`.

## Draft 1.153 resolved the earlier contradictions

Draft 1.152 had several self-contradictions that were previously accommodated
(expression-bodied function literals, `case pattern:` single-statement bodies,
a top-level `for` loop in a declaration-shorthand example, `Type.SizeOf` width,
duplicate `scoped`, arbitrary-expression monomorphization bases, and accessor
member-access wording). Draft 1.153 fixes all of these: the examples now use
block bodies, `MatchCase` uses braces, the loop is inside a function, `SizeOf`
returns 64 bits, duplicate `scoped` is illegal, `MonomorphizationExpression`
takes an `Identifier`, and accessor member access is explicitly allowed. ai-nsc
therefore no longer accepts the 1.152 tolerances.

## Inline-assembly examples are syntax fragments

The §5.2 examples write bare `assembly ...` statements at file scope, while the
prose requires `assembly` to appear inside a function body. The examples are
treated as syntax fragments (`NON_PROGRAM_FRAGMENTS` in
`tests/spec_examples.rs`), not as complete programs; the parser/checker enforce
the function-body placement rule.

## Type-constructor fragment

The `function(#integer(4), #integer(4))` block around line 9674 is a bare type
expression, not a program or statement; it is listed in
`NON_PROGRAM_FRAGMENTS`.

## Draft 1.153 example snippets that are not complete programs

A systematic audit of every `novasharp` example containing `fixed Main` found
that the examples below still fail semantic analysis for reasons that are
properties of the snippets, not compiler gaps:

- Lines around 3802, 4748, 4816, 6581, 6650, 6713, 6892, 8869: functions with a
  non-void return type omit a `return` statement (their bodies are demonstration
  fragments / contain `// ...`), so §5.11.2's return-path rule correctly rejects
  them.
- Line 10508 deliberately declares several `Main` overloads to illustrate the
  launcher rule ("exactly one Main"), so it is not a single executable.

The conformance test `only_documented_spec_programs_fail_semantics` keeps this
set closed: any newly failing example must be documented here.

## Parser assumptions (not contradicted by the spec, but worth knowing)

- `}` terminates a statement when it is followed by a line terminator before the
  next token (§2.8 says "only whitespace/comments after `}` in the same line");
  the parser approximates this with the line-gap check.
- Keywords are accepted as names wherever a name is expected (§2.4); `@` forces
  the keyword meaning, including at statement start (`@class Name { }`).
- The whitespace restrictions around `@` and `#` are lexer-level only.
- `MatchCase` guard-only form (`case when expr { }`) is accepted and treated as
  a wildcard pattern with a guard.

## Open spec questions that affect semantics

The per-section notes (`types.md`, `expressions.md`, `statements.md`,
`names-modules-functions.md`, `runtime-compiletime-builtins.md`) list the open
questions. The ones that shape the implementation most:

1. Runtime exceptions never cross a function boundary; `trap` is a lexical
   region.
2. Class layout, vtable/typeinfo layout and interface dispatch are unspecified
   beyond "instance carries typeinfo + vtable". ai-nsc picks a layout and records
   it in `docs/ABI.md`.
3. Function type equality includes `#` and `&`; the overload signature excludes
   them (Draft 1.153 makes this explicit), while the mangling hash records the
   contract markers.
4. Named arguments, defaults and `this` interact without a complete rule for
   candidate selection. ai-nsc resolves overloads by base argument types only
   and binds names after selection.
5. Inline assembly lowering details (operand constraint spelling, target syntax
   validation) are only partially specified; ai-nsc documents its choices in
   COVERAGE/AUDIT.
