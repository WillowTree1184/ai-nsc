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
