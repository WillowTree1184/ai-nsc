# Draft 1.12 inconsistencies observed while implementing

The specification is a moving draft. These are the places where a literal
reading of the grammar contradicts the prose or the spec's own examples, and
what ai-nsc does about each one. Line numbers refer to `Draft 1.12.md`.

| # | Spec location | Inconsistency | ai-nsc behaviour |
|---|---|---|---|
| 1 | L6583 (`MatchCase = "case" … Block`) vs Built-In Library L10348-10350 | The grammar requires a `Block` case body, but the official library writes `case is payload:Optional<T>.Value:` followed by a single `return …;` statement. | Parser accepts both `case pattern Block` and `case pattern : Statement` (the latter is recorded as tolerance, not as accepted grammar). |
| 2 | §2.5/§3.6.1 EBNF vs L1844-1845 | Array parameter expansion (`message[length: cardinal(4)]: cardinal(1)`, expanded to `message: array(length: cardinal(4)) cardinal(1)`) is used by the spec and the built-in library but not derivable from `ArrayTypeConstructor`. | Array dimensions accept a named binding `Name : Type` in addition to a size expression. |
| 3 | L5332-5336 example | The example writes a function literal as `(): Result<…> => …`, but `FunctionLiteral` has no `:` before the return type. | Parser accepts an optional `:` before a function-literal return type. |
| 4 | L7561 example (content line 19) | Function literals have expression bodies (`=> value`), while `FunctionLiteral` requires a `Block`. | Parser accepts either a block or a single expression body; sema treats the expression as an implicit return. |
| 5 | L10348-10350 vs L9750 area | Match cases with `:` are only one symptom; the Built-In Library also omits braces in several places (this is the only one hit by parsing). | Covered by #1. |
| 6 | §10.1 L8846-8851 vs §10.2 L8921, L8928, L8935 | The `MonomorphizationExpression` grammar allows `( Identifier \| Expression )` as the base and defines `Expression` as "the monomorphized expression", but the prose requires the monomorphized **identifier** to carry generic parameters (L8935) and makes the generic **declaration** the unit of monomorphization (§10.1 L8849). An arbitrary expression is thus syntactically legal but semantically impossible. | Parser accepts a postfix `Expression<…>`; sema requires the base to resolve to a generic named declaration and reports "expected a generic type or generic function before '<'" / "only generic types and generic functions can be monomorphized" otherwise. |
| 7 | §10.1 L8907-8913 example vs L8816 | The example's last case is a *negative* case: it asserts (in the trailing `// 错误：integer(4) 不属于 class 种类` comment) that instantiating `structure Bad<T: class>` with `integer(4)` must be an error, but the invalid code is commented out, so the parse-level conformance test cannot exercise it and the assertion exists only as prose. | ai-nsc **now enforces it**: the kind-keyword constraint `class` rejects `integer(4)` and reports "type argument 'integer(4)' does not satisfy constraint 'class' of generic parameter 'T'" (`tests/cli.rs::generic_constraint_violation_is_reported`). This is the behaviour the example asks for; the commented example itself is only parse-skipped. |

## Parser assumptions (not contradicted by the spec, but worth knowing)

- `}` terminates a statement when it is followed by a line terminator before the
  next token (§2.8 says "only whitespace/comments after `}` in the same line");
  the parser approximates this with the line-gap check.
- Keywords are accepted as names wherever a name is expected (§2.4); `@` forces
  the keyword meaning, including at statement start (`@class Name { }`).
- The whitespace restrictions around `@` and `#` are lexer-level only; the
  parser accepts `@ class` as an error at the lexer, but does not enforce other
  adjacency rules that the spec states only in prose.
- `MatchCase` guard-only form (`case when expr { }`) is accepted and treated as a
  wildcard pattern with a guard.

## Open spec questions that affect semantics (see per-section notes)

The per-section notes (`types.md`, `expressions.md`, `statements.md`,
`names-modules-functions.md`, `runtime-compiletime-builtins.md`) list 22–44 open
questions each. The ones that shape the implementation most:

1. Lexical exception model: runtime exceptions (divide-by-zero, out-of-range,
   allocation failure) cannot cross a function boundary; a callee's failure
   aborts. `trap` is a lexical region. (`statements.md` OQ-7.)
2. Class layout, vtable/typeinfo layout and interface dispatch are unspecified
   beyond "instance carries typeinfo + vtable". ai-nsc picks a layout and records
   it in `docs/ABI.md` when codegen lands. (`types.md` OQ on class layout.)
3. Function type equality excludes `&` but includes `#`, so an explicit cast can
   change the ABI. ai-nsc follows the type rules; codegen documents the ABI.
4. Named arguments, defaults and `this` interact without a complete rule for
   candidate selection. ai-nsc resolves overloads by argument types only and
   validates/binds names after selection. (`names-modules-functions.md`.)
5. `#`/`&` are required to distinguish overloads but are absent from the
   mangled name; ai-nsc will add the `#` marker to the hash input (recorded in
   `docs/ABI.md`) unless the spec settles it first.

| 8 | §3.6.2 | 指针减法结果为「元素个数」，但未规定该结果的具体整数类型（符号、宽度）。实现取 `integer(8)`（有符号 64 位），并在 `docs/AUDIT.md` #11 中记录。 |

## Draft 1.152 example snippets that are not complete programs

A systematic audit of every `novasharp` example containing `fixed Main` found
that the examples below still fail semantic analysis for reasons that are
properties of the snippets, not compiler gaps:

- Lines around 3784, 4729, 4797, 6408, 6476, 6538, 6717, 8651: functions with a
  non-void return type omit a `return` statement (their bodies are demonstration
  fragments / contain `// ...`), so §5.10.2's return-path rule correctly rejects
  them.
- Line 4229's `Main` ends without a `return` statement for the same reason.
- Line 10216 deliberately declares three `Main` overloads to illustrate the
  launcher rule ("exactly one Main"), so it is not a single executable.
- Line 7016/7078 were fixed (boolean condition literal inference) and are not in
  this list any more.

The conformance test `sema_clean_spec_programs_compile` excludes examples that
fail semantic analysis by design; the audit above distinguishes snippet
incompleteness from genuine gaps.

## Single-expression function-literal bodies

§2.7.9's grammar restricts a function-literal body to `Block | VerbatimBlock`,
but the specification's own function-type-constructor example (the block around
line 7808 of Draft 1.152) writes bodies such as `(value: integer(4)) => value`.
ai-nsc accepts a single expression after `=>` and treats it as an implicit
return; this is a deliberate accommodation for the draft's examples, recorded as
a deviation in `docs/COVERAGE.md`.
