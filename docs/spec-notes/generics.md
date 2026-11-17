# NovaSharp 泛型 / 单态化 — 规范摘录与实现笔记（Draft 1.12 §10）

Source: `docs/grammar/Draft 1.12.md`, §10 泛型, lines **8782–8949** (read in full:
2 `ebnf` blocks, 1 `novasharp` example block). Cross-references to other sections are
marked `[XREF]` and are used only to resolve dependencies.

Tags: each normative bullet ends with its spec line range, e.g. `[L8848-8851]`.
`MUST` = 必须, `MUST NOT` = 不得, `MAY` = 允许, `SHOULD` = 应当. Keywords keep the
spec spelling (`GenericParameters`, `Constraint`, `TypeKind`, `MonomorphizationExpression`).

For the surrounding name/function/modifier analysis see
`docs/spec-notes/names-modules-functions.md` §3.27–3.28 (and open questions 16–17
there); this note is the focused §10 extract plus what the compiler does with it.

---

## 1. Section map (line anchors)

| Section | Title | Lines | Content |
| --- | --- | --- | --- |
| §10 | 泛型 | 8782–8783 | heading |
| §10.1 | 泛型参数 | 8784–8914 | `GenericParameters`, `Constraint`, `TypeKind`; scope; allowed/forbidden positions; mono requirement; example |
| §10.2 | 单态化表达式 | 8916–8949 | `MonomorphizationExpression`; arity/constraint rules; compile-time semantics |

---

## 2. EBNF (verbatim, source order)

### 2.1 §10.1 泛型参数 — L8788–8797

```ebnf
GenericParameters = "<" GenericParameterDefinition { ";" GenericParameterDefinition } ">" .

GenericParameterDefinition = GenericParameter [ ":" Constraint { "," Constraint } ] .

GenericParameter = Identifier .

Constraint       = TypeKind | Identifier .

TypeKind = "class" | "structure" | "interface" | "enumeration" | "variant" | "union" | "basic" .
```

### 2.2 §10.2 单态化表达式 — L8920–8922

```ebnf
MonomorphizationExpression = ( Identifier | Expression ) "<" [ Type ] { ";" [ Type ] } ">" .
```

---

## 3. Normative rules

### 3.1 Generic parameters (§10.1)

- Definitions `[L8801-8808]`: `GenericParameters` is the parameter list;
  `GenericParameterDefinition` one parameter definition; `GenericParameter` its name
  (a *name* per §2.4, so keyword-named parameters use the §2.4 rules); `Constraint`
  is either a `TypeKind` keyword or a type `Identifier`; `TypeKind` is one of
  `class` / `structure` / `interface` / `enumeration` / `variant` / `union` /
  `basic`.
- Syntax `[L8812-8817]`: parameter definitions are separated by `;`; constraints by
  `,`; multiple constraints on one parameter have **AND** semantics; the parameter
  list follows the name. A kind-keyword constraint requires the argument to belong
  to that kind; a concrete-type constraint requires the argument to **inherit,
  implement, or equal** that type.
- Scope `[L8821-8824]`: a generic parameter's scope is the **declaration body**; it
  is accessible in that scope and is a **compile-time constant**; it may be used
  directly in compile-time code `[XREF §11.7]`.
- Allowed positions `[L8828-8837]`: generic parameters may appear only after the
  name of `fixed`, `linkable`, `class`, `structure`, `interface`, `union`,
  `variant`, a compile-time function name `[XREF §11.7]`, or `extension`.
- Forbidden `[L8838-8844]`: `enumeration` declarations **MUST NOT** carry generic
  parameters; field definitions, `local`, `heap` and `operator` **MUST NOT** carry
  them; an `accessor` **MUST NOT declare** generic parameters, but **MAY borrow**
  those of its enclosing declaration.
- Monomorphization `[L8848-8851]`: it **MUST** run at compile time; the unit is the
  declaration that carries generic parameters; it **MUST** be initiated through a
  monomorphization expression; if it fails, the compiler **MUST** report an error.
- Interaction with type syntax `[XREF §3.1 L1480-1483]`: a generic parameter
  **MUST NOT** appear in a type (parameters are allowed only in declarations); when
  a type needs generic actual arguments, they **MUST** be carried through a
  monomorphization expression (§10.2).
- Generic extension targets `[XREF §3.8.3 L3448-3455]`: a generic target **MUST**
  start with an identifier and its type arguments **MUST** be written as generic
  parameters; those parameters **MAY** carry constraints, and the extension applies
  only to instances that satisfy them. Unbound identifiers in the type arguments
  **MUST** be interpreted as generic parameters.

### 3.2 Monomorphization expressions (§10.2)

- Definitions `[L8926-8931]`: `MonomorphizationExpression` is an expression that
  monomorphizes a generic identifier; `Identifier` is the monomorphized identifier;
  `Expression` is the monomorphized expression; `Type` is a generic actual argument.
  Actual arguments are separated by `;`; an argument **MAY** be omitted, and an
  omitted argument denotes the **empty type** `[XREF §3.6]`.
- Type rules `[L8935-8937]`: the monomorphized identifier **MUST** carry generic
  parameters; the number of actual arguments **MUST equal** the number of generic
  parameters; each actual argument **MUST** satisfy the corresponding parameter's
  constraints.
- Behaviour `[L8941-8942]`: monomorphization **MUST** run at compile time; failure
  ⇒ the compiler **MUST** report an error.
- Result `[L8946-8947]`: the expression's value is the monomorphized result of the
  identifier, and the **kind** of the result **MUST** be the same as the kind of
  the monomorphized identifier.
- Volume optimization of monomorphized instances is delegated to §14.3.2
  `[L8949, XREF]` (not implemented).

### 3.3 Example (§10.1, L8853–8914)

`structure Box<T: structure>`, `structure Pair<T: class; U>` (multiple parameters),
`structure SortedList<T: class, IComparable>` (multiple constraints),
`structure BasicBox<T: basic>`, `fixed Echo<T>(value: T): T`, and the explicit
instantiation `Echo<integer(4)>(42)` `[L8855-8898]`. Commented illegal forms:
a field carrying generic parameters `[L8900-8905]` and the constraint violation
`Bad<integer(4)>` for `structure Bad<T: class>` `[L8907-8913]`.

---

## 4. Implementation notes

Implemented in `src/sema/generics.rs`; the AST cloning lives in `src/ast_clone.rs`.
Generic declarations are stored as **templates** (`TypeDef.generic_parameters` /
`FunctionInfo.generic_parameters` non-empty, `template: None`); they are never
resolved directly and are skipped by pass 2 (which computes structure layouts and
resolves members), by vtable building, by body checking (templates have
`body: None`) and by codegen. An instance is a deep clone of the declaration
subtree resolved under a substitution environment (`generic_env`), cached per
`(declaration node, argument list)`, with `template: Some(template_index)` and
empty `generic_parameters`.

| Rule | Where enforced |
| --- | --- |
| allowed positions / forbidden declarations | parser (`EnumerationDecl` has no generic-parameter field, so a generic `enumeration` is a parse error); `FixedDecl`/`LinkableDecl`/`ClassDecl`/`StructureDecl`/`InterfaceDecl`/`VariantDecl`/`UnionDecl` grammar; `declare_generic_function` / `declare_generic_linkable` require a function type and a body `[L8838-8844]` |
| kind constraints (`class`, `structure`, `interface`, `enumeration`, `variant`, `union`, `basic`) | `check_generic_constraints` → `type_matches_kind`; `basic` accepts `boolean`, `integer(N)`, `float(N)` only |
| named-type constraints (inherit / implement / equal) | `check_generic_constraints` → `Sema::is_assignable` for the named type; for generic **functions**, unknown constraint names are reported when the template is declared; for generic **types** an unknown name is currently treated as satisfied (no cascade), so it is not diagnosed at the declaration |
| AND semantics, separators | parser (`GenericParameter.constraints` list); every constraint checked in `check_generic_constraints` |
| arity equality | `instantiate_type` / `instantiate_function` report `expects N type argument(s), found M` |
| empty actual argument = empty type | `resolve_type_arguments` maps an omitted argument to `TypeId::VOID` |
| compile-time failure ⇒ error | argument-count and constraint failures report a diagnostic and return `None`; an accepted argument list is registered in the cache before its clone is resolved so that recursive uses terminate, and member-resolution errors are ordinary diagnostics that fail the unit |
| monomorphization positions | `check_monomorphization` from type positions (`NamedType.arguments`), constructions, calls, member constructions and `is` targets |
| result-kind equality | instances are created with the template's `TypeDefKind` (`class`/`structure`/… preserved) |
| allowed/forbidden declaration positions | enforced by the parser/AST shape where possible; field/`local`/`heap`/`operator`/`accessor` generic parameters are not representable in the AST |
| generic-parameter references / scope | a bare `NamedType` whose name matches an active `generic_env` entry resolves to the substituted `TypeId` (`lookup_generic_parameter`, innermost environment first, so parameters shadow outer names) `[L8821-8824]`; in a generic extension target, unbound identifiers in the type arguments become the extension's parameters `[XREF §3.8.3 L3455]` |
| module-level `extension for Generic<T>` | `register_generic_extension` records a `PendingGenericExtension`; `apply_generic_extension` instantiates it per instance and skips instances that violate the extension's own constraints |
| per-instance symbols | instance qualified names are `Template<arg;…>`; method mangling uses the instance name and parameter spellings, so two instantiations coexist (§13.2) |

**Deferred / not implemented.**

- Generic **methods** declaring their own type parameters (`fixed Foo<U>(...)`
  inside an extension): `create_method` records extension methods with empty
  `generic_parameters` and there is no function-instantiation path for members, so
  such calls are rejected with a diagnostic.
- **Array extension targets** (`extension for array(...)`): `resolve_extension_target`
  reports "array extension targets" as not implemented.
- Generic **enumeration declarations**: forbidden by §10.1 `[L8838]` and the parser
  does not accept them (unchanged).
- Constructed constraints (e.g. `T: IComparable<integer(4)>`) are not derivable
  from `Constraint = TypeKind | Identifier` (see open question 16 in
  `names-modules-functions.md`); only bare names/kinds are supported.
- Compile-time use of generic parameters (`#(...)`, `#{...}`, §11.7) is deferred
  together with §11 compile-time execution; see `docs/COVERAGE.md` §11.
- Monomorphization volume optimization (§14.3.2) is not implemented.
