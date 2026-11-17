# Grammar specification

Put the NovaSharp grammar specification in this directory. Any plain-text format
works (`.md`, `.ebnf`, `.txt`, `.y`, ...); if it is a PDF or an image, please also
provide a text extract so it can be quoted and diffed.

The spec is the single source of truth for:

- the token inventory (keywords, operators, literals, comments, identifiers)
- declarations, statements and expression forms
- operator precedence and associativity
- type syntax and the type system rules
- error-recovery requirements, if any

Once it lands it will be distilled into `docs/GRAMMAR.md`, which stays in sync
with the lexer and parser. The original specification is kept here unchanged.
