# NovaSharp §5 语句 — Implementation Note

Source: `docs/grammar/Draft 1.1.md`, lines **5444–7214** (§5 语句, i.e. §5.1–§5.10.4). Normative
extraction only: each bullet is tagged with the spec lines it derives from, and nothing is invented —
where the spec is silent or self-contradictory the item appears in §7 instead of a decision.
Cross-references read for disambiguation (outside the range): §2.5 `[L400–463]`, §2.8 `[L1313–1335]`,
§12.1 `[L9406–9478]`, §12.2 `[L9505–9584]`, §4.8 `[L4225–4267]`, §2.7.10–2.7.12 `[L1188–1312]`.

Legend: `S-…` = extracted rule; `D-…` = deduction (§6); `OQ-…` = open question (keywords verbatim).

## 1. Section map

| § | Title | Lines | Content |
|---|---|---|---|
| 5 | 语句 | 5444–5528 | `Statement`/`FileBody`/`ExpressionStatement`; statement-level rules |
| 5.1 | 代码块 | 5530–5581 | `Block`, 类型体, statement scope, `#{…}` exception |
| 5.2 | `local` | 5583–5661 | variable definition, inference, storage class, scope |
| 5.3 | `fixed` | 5663–5752 | constant definition, initialization, generic function constants |
| 5.4 | `heap` | 5754–5857 | allocation unit, allocator expression, `DefaultAllocator`, `manual` |
| 5.5 | `linkable` | 5859–5936 | external symbol exposure, symbol name, allowed positions |
| 5.6 | use 语句 | 5938–5987 | member import into the current scope |
| 5.7 | 赋值语句 | 5989–6105 | `SetStatement`, `CompoundSetStatement`, targets, eval order |
| 5.8 | dispose 语句 | 6107–6201 | explicit release, destructor order, double-dispose |
| 5.9 | 逻辑控制流 | 6203–6893 | umbrella for §5.9.1–§5.9.8 |
| 5.9.1 | 逻辑控制流名称 | 6205–6244 | `: Name` label form, positions, uniqueness, scope |
| 5.9.2 | if 语句 | 6246–6318 | `if` / `else if` / `else` |
| 5.9.3 | while 语句 | 6320–6381 | pre-test loop |
| 5.9.4 | do while 语句 | 6383–6433 | post-test loop; condition inside block scope |
| 5.9.5 | for 语句 | 6435–6574 | array / `IIterator` / `IIterable` / range traversal |
| 5.9.6 | match 语句 | 6576–6765 | patterns, guards, continue-to-next-case, exhaustiveness |
| 5.9.7 | break 语句 | 6767–6822 | labelled / unlabelled exit |
| 5.9.8 | continue 语句 | 6824–6893 | labelled / unlabelled continue, default behaviour |
| 5.10 | 控制流 | 6895–7213 | umbrella for §5.10.1–§5.10.4 |
| 5.10.1 | 异常处理 | 6897–6929 | exception model, runtime exception set, trap correspondence |
| 5.10.2 | return 语句 | 6931–7011 | return value, caller-provided storage, all-paths rule |
| 5.10.3 | throw 语句 | 7013–7079 | local throw, nearest intercepting `trap` |
| 5.10.4 | trap 语句 | 7081–7213 | `trap`/`catch`/`finally`, matching, pending-action restore |

20 `ebnf` blocks and 19 `novasharp` blocks, all listed by line range below (§2, §5).

## 2. Consolidated EBNF (verbatim, in spec order)

Literal terminals in double quotes; `{ }` = repetition, `[ ]` = optional, `.` = production end.
No production here contains a statement terminator; termination is delegated to §2.8 (`;` or `}`).

### 2.1 §5 语句 — statement alternatives `[L5448–5484]`
```ebnf
Statement = LocalDefinition
          | HeapDefinition
          | OperatorDefinition
          | FixedDefinition
          | LinkableDefinition
          | FieldDefinition
          | AccessorDefinition
          | SetStatement
          | CompoundSetStatement
          | UseStatement
          | ModuleDeclaration
          | ClassDeclaration
          | StructureDeclaration
          | InterfaceDeclaration
          | EnumerationDeclaration
          | VariantDeclaration
          | UnionDeclaration
          | ExtensionDeclaration
          | CompileTimeBlock
          | CompileTimeIf
          | IfStatement
          | WhileStatement
          | DoWhileStatement
          | ForStatement
          | MatchStatement
          | BreakStatement
          | ContinueStatement
          | ReturnStatement
          | ThrowStatement
          | TrapStatement
          | DisposeStatement
          | ExpressionStatement .

FileBody = { Statement } .
ExpressionStatement = FunctionCallExpression .
```
### 2.2 §5.1 代码块 — `Block` `[L5534–5536]`
```ebnf
Block = "{" { Statement | Block } "}" .
```
### 2.3 §5.2 local — variable definition `[L5587–5591]`
```ebnf
LocalDefinition = { Modifier } "local" LocalItem { "," LocalItem } .

LocalItem       = Name [ ":" Type ] [ "=" Expression ] .
```
### 2.4 §5.3 fixed — constant definition `[L5671–5673]`
```ebnf
FixedDefinition = { Modifier } "fixed" Name [ GenericParameters ] [ ":" Type ] [ "=" Expression | Block ] .
```
### 2.5 §5.4 heap — heap allocation definition `[L5758–5760]`
```ebnf
HeapDefinition = { Modifier } "heap" [ "(" Expression ")" ] Name [ ":" Type ] [ "=" Expression ] .
```
### 2.6 §5.5 linkable — external symbol definition `[L5863–5866]`
```ebnf
LinkableDefinition = { Modifier } "linkable" [ "(" StringLiteral ")" ] FunctionShorthand
                   | { Modifier } "linkable" [ "(" StringLiteral ")" ] Name [ GenericParameters ] [ ":" Type ] [ "=" Expression | Block ] .
```
### 2.7 §5.6 use 语句 — member import `[L5942–5944]`
```ebnf
UseStatement = "use" Identifier { "," Identifier } .
```
### 2.8 §5.7 赋值语句 — assignment / compound assignment `[L5993–6001]`
```ebnf
SetStatement          = AssignmentTarget "=" Expression .

CompoundSetStatement  = AssignmentTarget ( "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ) Expression .

AssignmentTarget      = Identifier
                      | IndexAccessExpression
                      | PointerMemberAccessExpression .
```
### 2.9 §5.8 dispose 语句 — explicit release `[L6111–6113]`
```ebnf
DisposeStatement = "dispose" ( Identifier | PointerMemberAccessExpression ) .
```
### 2.10 §5.9.1 逻辑控制流名称 — control-flow label `[L6209–6211]`
```ebnf
LogicalControlFlowName = ":" Name .
```
### 2.11 §5.9.2 if 语句 `[L6250–6252]`
```ebnf
IfStatement = "if" Expression Block { "else" "if" Expression Block } [ "else" Block ] .
```
### 2.12 §5.9.3 while 语句 `[L6324–6326]`
```ebnf
WhileStatement = "while" Expression [ ":" Name ] Block .
```
### 2.13 §5.9.4 do while 语句 `[L6387–6389]`
```ebnf
DoWhileStatement = "do" [ ":" Name ] Block "while" Expression .
```
### 2.14 §5.9.5 for 语句 `[L6439–6445]`
```ebnf
ForStatement = "for" [ "&" ] Name ":" Type "in" TraverseTarget [ ":" Name ] Block .

TraverseTarget = Expression | Range .

Range = Expression [ ".." Expression ] ( ".." | "..!" ) Expression .
```
### 2.15 §5.9.6 match 语句 `[L6580–6597]`
```ebnf
MatchStatement  = "match" Expression [ ":" Name ] "{" { MatchCase } "}" .

MatchCase       = "case" ( Pattern [ Guard ] | Guard ) Block .

Pattern            = EqualPattern
                   | ComparisonPattern
                   | IsPattern
                   | WildcardPattern .

EqualPattern       = Expression .
ComparisonPattern  = ComparisonOperator Expression .
ComparisonOperator = "!=" | ">" | "<" | ">=" | "<=" .
IsPattern          = "is" [ Name ":" ] IsTarget .
WildcardPattern    = "wildcard" .

Guard           = "when" Expression .
```
### 2.16 §5.9.7 break 语句 `[L6771–6773]`
```ebnf
BreakStatement = "break" [ Name ] .
```
### 2.17 §5.9.8 continue 语句 `[L6828–6830]`
```ebnf
ContinueStatement = "continue" [ Name ] .
```
### 2.18 §5.10.2 return 语句 `[L6935–6937]`
```ebnf
ReturnStatement = "return" [ Expression ] .
```
### 2.19 §5.10.3 throw 语句 `[L7017–7019]`
```ebnf
ThrowStatement = "throw" Expression .
```
### 2.20 §5.10.4 trap 语句 `[L7085–7093]`
```ebnf
TrapStatement  = "trap" TrapBody { "catch" CatchCondition CatchBlock } [ "finally" FinallyBlock ] .

TrapBody       = Block .
CatchBlock     = Block .
FinallyBlock   = Block .
CatchCondition = [ Name ":" ] Type
               | "wildcard" .
```

`Block` is **not** a `Statement` alternative — nested bare blocks come only from the alternative
*inside* `Block` `[L5535]`; `ExpressionStatement` = `FunctionCallExpression` `[L5483]`.

## 3. Semantic rules

### 3.1 Statement level `[L5446–5528]`
- **S-5-1/2/3** A statement is a syntactic unit that can stand alone in a code block or file body
  `[L5446]`; `Statement` is the 32-way alternative of §2.1 `[L5448–5480, L5488]`; the file body is the
  top-level statement sequence `[L5482, L5524]`.
- **S-5-4..7** The file body **must** constitute a statement scope `[L5525]`; every statement **must**
  be terminated by a statement terminator (§2.8) `[L5526]`; member definitions **must not** be
  regarded as statements `[L5527]` — the `FieldDefinition`/`AccessorDefinition`/`OperatorDefinition`
  alternatives are reachable only in member position and are inert in a function body; an expression
  statement **must** evaluate its function call and **discard the return value** `[L5528]`, so a
  non-call expression is not a statement.
- **S-5-8** Remaining alternatives are owned by §5.2–§5.10, §4.9, §3.7.x, §3.8.x, §7, §11.4, §11.6
  `[L5489–5520]`.

### 3.2 Code blocks and scopes `[L5532–5558]`
- **S-5.1-1/2/3** `Block` is a declaration-sequence body `[L5535, L5544]`; **类型体** covers
  class/structure/interface/enumeration/variant/union bodies **and extension blocks** `[L5545]`; a
  type body **must** consist of a declaration sequence `[L5546]`.
- **S-5.1-4/5** Conditional compilation **may** appear, and its selected branch **must** satisfy the
  body's member rules `[L5547]`; class/structure bodies **must directly contain only** field
  definitions, module declarations and extension declarations `[L5548]` — constants, accessors,
  operators, methods and constructors/destructors **must** live in extension blocks `[L5549]`.
- **S-5.1-6/7** Interface bodies **must not** contain extension declarations (members are direct
  contract) `[L5550]`; enumeration/variant/union bodies **may** `[L5551]`, with states, state payloads
  and member definitions declared directly and all other members in extension blocks `[L5552]`.
- **S-5.1-8..11** A statement terminator **must not** be used outside statements `[L5553]`; `Block`
  **must** constitute a statement scope `[L5557]`; the `Block` of `#{ … }` **must not** constitute a
  scope — its declarations belong to the enclosing scope (§11.4, `[L7243]`) `[L5558]`; a nested bare
  `Block` needs no `;`, its closing `}` terminating it `[L5570, L6298–6301]`.

### 3.3 `local` `[L5583–5647]`
- **S-5.2-1/2** `local` acquires **instance storage in the current scope** `[L5585]`; the modifier
  list **must** precede it `[L5604]`.
- **S-5.2-3/4/5/6** Per `LocalItem` at least one of `:` and `=` **must** appear `[L5605]` (bare
  `local bad;` is the mandated error `[L5645–5647]`); items **must** be defined in written order
  `[L5606]` and each **must** be inferred/initialized independently `[L5607]`; §2.5 shorthand applies
  `[L5608]`; default accessibility §9.1 `[L5609]`.
- **S-5.2-7/8/9/10/11** Storage **must** be acquired before initialization completes `[L5613]`; the
  initializer type **must be** the variable type or implicitly convertible `[L5614]`; with `=`, storage
  **must** be overwritten by it (defined as initialization) `[L5615–5616]`; without `=`, the variable
  **must be uninitialized** (§12.1) `[L5617]`; without `:`, the type **must** be inferred from the `=`
  result `[L5618]`.
- **S-5.2-12/13** With `:`, the type **must be** that `Type` `[L5622]` and **must not** be the empty
  type `[L5623]`.
- **S-5.2-14/15/16/17** Duration **must be** scope storage duration `[L5627]`; scope **must be** the
  declaring scope `[L5628]`; in a **function body** ⇒ **must** be on the stack `[L5629]`, in a **module
  or file body** ⇒ **must** be in the global data area `[L5630]`; heap duration is §5.4 `[L5631]`.
- **S-5.2-18** Illegal: `local` in a type body — use field definitions `[L5649–5654]`.

### 3.4 `fixed` `[L5663–5711]`
- **S-5.3-1/2/3** `fixed` is the identifier kind **not assignable after initialization** `[L5665]`
  (deliberately unlike C# `fixed` `[L5667–5669]`); form per §2.4 `[L5672]`.
- **S-5.3-4** Modifiers **must** precede `fixed` `[L5687]`; §2.5 shorthand applies `[L5688]`.
- **S-5.3-5/6** In an interface body the constant **must not** be initialized `[L5692]`; elsewhere it
  **must** be initialized — exactly one of `=` and `Block` `[L5693]`.
- **S-5.3-7/8/9** Non-function constant initialization and read-only-section rules are §14.2 `[L5694]`;
  `uninitialized` **must not** be a constant initializer `[L5695]`; without `:`, the type **must** be
  inferred from `=` `[L5696]`.
- **S-5.3-10/11** Constants **may only be assigned via initialization** `[L5697]` (assignment to one is
  illegal `[L6099–6101]`); the initializer type **must be** the constant type or implicitly convertible
  `[L5698]`.
- **S-5.3-12/13/14/15** Generic parameters **only** in function constants `[L5702]`; a **function
  constant** has a function type §3.6.3 `[L5706]`; a **method** is a function constant whose parameter
  list contains `this` §8.4 `[L5707]`; `fixed` **may** carry `static` §9.15 `[L5711]`.
- **S-5.3-16** Mandated errors: missing initializer `[L5742–5744]`; initialized interface constant
  `[L5746–5751]`.

### 3.5 `heap` `[L5754–5817]`
- **S-5.4-1/2/3** `heap` acquires instance storage **on the heap** `[L5756]`; the allocator expression
  precedes the name `[L5759]`; `manual` §9.9 marks storage **not governed by NOLAA** `[L5766]`.
- **S-5.4-4** Modifiers **must** precede `heap` `[L5773]` (`heap bad manual: …` mandated error
  `[L5854–5856]`); at least one of `:` and `=` **must** appear `[L5774]`; shorthand applies `[L5775]`;
  default accessibility §9.1 `[L5777]`.
- **S-5.4-5/6** With `:`, the type **must be** that `Type` `[L5781]` and **must not** be empty
  `[L5782]`; without it, **must** be inferred from `=` `[L5783]`; the storage size **must exactly fit
  a value of that type** `[L5784]`.
- **S-5.4-7/8** The allocator expression's type **must be `IAllocator`** §15.2 `[L5788]` and it **must
  be evaluated exactly once** `[L5789]`.
- **S-5.4-9/10/11/12** Omission **must** use global `DefaultAllocator` `[L5790]`, which **must be** a
  global constant of type `IAllocator` `[L5791]`, **must** exist (else error `[L5792]`), **must** be
  provided by developer or platform `[L5793]`, and **must** be initialized before the first
  allocator-omitting `heap` `[L5794]`.
- **S-5.4-13/14/15/16** The **allocation unit must record the actual allocator** `[L5795]`;
  `Allocate`→`Value` ⇒ **must** use the pointer `[L5796]`; →`Alternative` ⇒ **must** raise
  `AllocateFailedException` `[L5797]`, in which case the name **must not** be bound and the initializer
  **must not** run `[L5798]`.
- **S-5.4-17/18/19** Heap storage **must** be acquired before initialization completes `[L5802]`; with
  `=`, it **must** be overwritten by the initializer (its initialization) `[L5803–5804]`; without `=`,
  it **must** be uninitialized heap storage `[L5805]`.
- **S-5.4-20/21** Scope **must** equal a `local`'s in the same position `[L5809]`; at scope end the
  heap storage **must not** be released `[L5810]`.
- **S-5.4-22/23/24/25** Without `manual`, release **must** be decided by NOLAA `[L5814]`; with
  `manual`, the developer **must** release it `[L5815]`; both **may** be released by `dispose`
  `[L5816]`; after an explicit release NOLAA **must not** release again `[L5817]`.

### 3.6 `linkable` `[L5859–5926]`
- **S-5.5-1/2** `linkable` **guarantees exposure as an external symbol** `[L5861]`; two alternatives
  (with `FunctionShorthand`, or with a `Name`), both taking an optional `"(" StringLiteral ")"` symbol
  name `[L5864–5865]`.
- **S-5.5-3/4/5/6/7** Modifiers **must** precede `linkable` `[L5882]`, which **must** precede the name
  `[L5883]`; it **must not** co-occur with `fixed` `[L5884]`; the value **must be a constant**
  `[L5885]`, **must not** be assigned `[L5886]`, **may** be of any type `[L5887]`; the modifier list
  **must contain only accessibility modifiers** `[L5888]`.
- **S-5.5-8/9/10/11** Without `=`/`Block` the symbol **must** be provided externally `[L5892]`; if also
  not a `FunctionShorthand`, `:` **must** appear `[L5893]`; with `Block`, the type **must** be a
  function type `[L5894]`; with `=`, the expression type **must be** `Type` or implicitly convertible
  `[L5895]` and the definition **must be** constant initialization `[L5896]`.
- **S-5.5-12/13/14** No `("…")` argument ⇒ symbol name = the `Name` `[L5900]`; with it ⇒ the literal's
  content, verbatim `[L5901]` (`"Total"` `[L5935]`); the name **must not** be governed by
  accessibility `[L5902]`.
- **S-5.5-15/16/17/18** The symbol **must** have **external linkage** `[L5906]`, **must not** be
  internalized `[L5907]`, **must not** be eliminated for having no user `[L5908]`; the source **must
  not** carry a library name `[L5909]`.
- **S-5.5-19/20** `linkable` **only** guarantees exposure `[L5913]` — never import, library
  resolution, or link direction `[L5914]`; runtime libraries are the linker's job `[L5915]`,
  compile-time libraries come from compiler command arguments `[L5916]`.
- **S-5.5-21/22/23** Allowed at file top level, in a module body, or in an extension block `[L5920]`;
  **must not** appear in a function body or as a **direct** type-body member `[L5921]`; **must not** be
  named `Construct`/`Destruct` `[L5922]`.
- **S-5.5-24** Side effects introduced by `linkable` are the developer's responsibility; behaviour
  beyond the external boundary is unspecified and undiagnosed `[L5924–5926]`.

### 3.7 `use` 语句 `[L5938–5960]`
- **S-5.6-1/2** A `use` statement **must** introduce the members of every comma-separated target into
  the current scope `[L5943, L5953]` and **must** be terminated by `;` `[L5954]`.
- **S-5.6-3/4/5** It **may** appear in any code block `[L5958]`, **must** affect only code following
  it in its own scope `[L5959]`, and **must not** be hoisted `[L5960]`.

### 3.8 赋值语句 `[L5989–6035]`
- **S-5.7-1/2** Assignment overwrites the memory of the assignment target with the target-value
  expression `[L5991]`; both statement forms **must appear only in a function body**
  `[L6006, L6008]`.
- **S-5.7-3/4/5/6** Targets are `Identifier`, `IndexAccessExpression` §4.4.3,
  `PointerMemberAccessExpression` §4.4.2 `[L5998–6000, L6013–6014]`; an identifier target **must**
  point to an accessible identifier assignable in the current context `[L6011]` and **must** point to a
  variable §5.2, heap identifier §5.4, field §3.8.1 or accessor §3.8.2 `[L6012]` — constants §5.3 and
  linkable values §5.5 are excluded `[L5886, L6099–6101]`.
- **S-5.7-7/8/9** The value type **must be** the target's type or implicitly convertible `[L6019]`;
  the assignment **must overwrite the target's memory** `[L6020]` — value-type target ⇒ **instance
  body** `[L6021]`, reference-type ⇒ **the reference** `[L6022]`; a construct-expression value **must
  be constructed into the target's storage** §4.6 `[L6023]`.
- **S-5.7-10/11/12/13** Compound assignment operates on the target's current value with the
  target-value expression and overwrites the target `[L6027]`, **must follow the target type's binary
  operator** §4.9.2 `[L6028]`, **must** evaluate the target exactly once `[L6029]`, and **must not** be
  regarded as `a = a op b` — its order is as specified here `[L6030]`.
- **S-5.7-14/15** The target's storage **must be determined before** the value expression `[L6034]`;
  for compound assignment the current value **must be read after** the value expression is evaluated
  `[L6035]`; operators: `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `&=`, `|=`, `^=`, `<<=`, `>>=`
  `[L5994–5996]`.

### 3.9 `dispose` 语句 `[L6107–6154]`
- **S-5.8-1/2** `dispose` destroys heap storage `[L6109]`; target is `Identifier` or
  `PointerMemberAccessExpression` `[L6112]`; it **must appear only in a function body** `[L6118]`.
- **S-5.8-3/4/5/6/7** The target **must** point to a heap allocation unit §5.4/§12.2
  `[L6121, L6126]`, **must not** point to a scope-storage-duration instance `[L6127]` (`dispose
  local;` mandated error `[L6192–6195]`), **must not** be a constant `[L6128]`, **must** be accessible
  `[L6122]`; those determinations **must** be the compiler's `[L6129]`.
- **S-5.8-8/9/10** Destroy per the target type's lifetime rules §3.7.1/§3.7.2 `[L6133]`; **destructor
  first, then reclaim** `[L6134]`; an incomplete destructor **must not** be followed by reclaim
  `[L6135]`.
- **S-5.8-11/12** Reclaim **must** use the `IAllocator.Deallocate` recorded in the unit `[L6136]`;
  `Alternative` ⇒ **`DeallocateFailedException`** `[L6137]`.
- **S-5.8-13/14** Release **must** cover only the pointed-to unit, never transitively other units the
  instance references `[L6138–L6139]`; this is **显式释放 / explicit release** `[L6140]`.
- **S-5.8-15/16/17** After explicit release NOLAA **must not** release again `[L6144]`; a
  `manual heap` unit **must** be released by the developer via `dispose` `[L6145]`; an earlier-than-safe
  -point dispose ⇒ **Early-Dispose Warning** `[L6146]`.
- **S-5.8-18/19/20/21** A destroyed instance **must not** be read/written `[L6150]` nor destroyed
  again `[L6151]` (violation ⇒ **Double-Dispose Warning** `[L6152]`); undecidable cases are dangerous
  and **should** warn `[L6153–6154]`; a `local` whose value references a heap unit **can** be disposed
  (`dispose made;` `[L6185–6187]`) — the rule concerns the pointed-to unit, not the identifier's
  storage form.

### 3.10 Logical control flow names `[L6205–6244]`
- **S-5.9.1-1/2/3** Form `":" Name` `[L6210]`, attached **only** to `while`, `do while`, `for`, `match`
  `[L6216–6220]` — `if` is excluded — and **must** sit between header and `Block` `[L6225]`: `while`
  after the condition `[L6226]`, `do while` after `do` `[L6227]`, `for` after the traversal target
  `[L6228]`, `match` after the matched expression `[L6229]`.
- **S-5.9.1-4/5** The same name **must not** occur twice in one scope `[L6233]`; an inner name **may**
  equal an outer one and **shadows** it `[L6234]`.
- **S-5.9.1-6/7** The name's scope is **all `Block`s of that flow** `[L6238–6239]`; a
  `break`/`continue` name **must resolve to the innermost same-named flow** `[L6243]`.

### 3.11 `if` `[L6246–6278]`
- **S-5.9.2-1..5** `if` **must** appear in a function body `[L6257]`; the condition **must be**
  `boolean` or implicitly convertible `[L6259]` (`if 1`, `if "text"` are errors `[L6294–6296,
  L6310–6312]`) and **must not** access identifiers defined in the `Block` `[L6260]`
  (`if x { local x = 1; }` is an error `[L6303–6308]`); parentheses **not required**, braces **must**
  be used `[L6265]`; behaviour: test, `true` ⇒ run the block and exit, else each `else if` in order,
  else the final `else`, else exit `[L6269–6278]`; any number of `else if`, at most one trailing
  `else` `[L6251]`.

### 3.12 `while` `[L6320–6347]`
- **S-5.9.3-1..5** `while` **must** appear in a function body `[L6331]`; the condition **must be**
  `boolean`/implicitly convertible `[L6333]` and **must not** access `Block` identifiers `[L6334]`;
  parentheses not required, braces required `[L6340]`; behaviour: test; `true` ⇒ execute `Block` and
  repeat the test, `false` ⇒ exit `[L6344–6347]`.

### 3.13 `do while` `[L6383–6410]`
- **S-5.9.4-1..7** `do while` **must** appear in a function body `[L6394]`; the condition **must be**
  `boolean`/implicitly convertible `[L6396]`, it **may** access `Block` identifiers `[L6397]`, and
  **the `Block`'s scope must include the condition expression** `[L6398]` — the opposite of
  `if`/`while`; behaviour: execute `Block`, test, `true` ⇒ repeat, else exit (body always runs once)
  `[L6408–6410]`; parentheses not required, braces required `[L6404]`; the name precedes the block
  `[L6388]`.

### 3.14 `for` `[L6435–6521]`
- **S-5.9.5-1/2/3** Form per §2.14 `[L6440]`; `&` is the **reference marker** and its presence
  **determines** a reference iteration variable `[L6452–6454]`; `..!` **must be lexed as a single token
  by maximal munch** `[L6457–6458]`.
- **S-5.9.5-4/5/6** Range: first `Expression` = **start**, optional second = **step**, last = **end**
  `[L6459–6461]`; the second `Name` is the flow name `[L6462]`; the iteration type **may** use §2.5
  shorthand `[L6464]`; `for` **must** appear in a function body `[L6450]`.
- **S-5.9.5-7/8** The traversal target **must** be one of, matched **in this order** when several apply
  `[L6468–6474]`: (1) an array-type expression §3.6.1; (2) an expression implicitly convertible to an
  array type; (3) a class instance implementing `IIterator`; (4) one implementing `IIterable`; (5) a
  range.
- **S-5.9.5-9** Array traversal: a reference variable **must** reference the element, otherwise it
  **must** be a copy `[L6478–6479]`.
- **S-5.9.5-10/11/12** `IIterator` `[L6483–6486]`: (1) call `Next()`; (2) status `Value` ⇒ assign the
  payload, execute `Block`, go to (1); (3) status `Null` ⇒ exit; the payload type **must be** the
  iteration type or implicitly convertible `[L6487]`; `IIterator` is a global Built-In Library
  interface §15.2 `[L6488–6489]`.
- **S-5.9.5-13/14** For `IIterable`, **first obtain its `Iterator` accessor**, then apply the iterator
  rules `[L6493]`; it too is a global Built-In Library interface `[L6494–6495]`.
- **S-5.9.5-15/16** `..` ⇒ left-closed right-closed `[L6501]`; `..!` ⇒ left-closed right-open
  `[L6502]`; step omitted ⇒ start ≤ end gives **+1**, otherwise **−1** `[L6503–6505]`.
- **S-5.9.5-17/18** Range traversal: assign start; while the traversal condition holds, execute
  `Block`, add the step, retest; else exit `[L6509–6514]`. Condition `[L6518–L6519]`: closed ⇒
  `step > 0 && i <= end` or `step < 0 && i >= end`; half-open ⇒ `step > 0 && i < end` or
  `step < 0 && i > end`.
- **S-5.9.5-19/20/21** Step and end are computed **in real time** and **may** be optimized
  `[L6520–6521]`; the iteration variable **must not** be accessible outside `[L6565–6568]`; ranges
  **must** have consistent types (`0.."text"` mandated error `[L6561–6563]`).

### 3.15 `match` `[L6576–6667]`
- **S-5.9.6-1..6** Form per §2.15 `[L6581–6596]`; `ComparisonOperator` is exactly `!=`, `>`, `<`,
  `>=`, `<=` — `==` is **not** one, equality being the bare `EqualPattern` `[L6592]`; `match` **must**
  appear in a function body `[L6602]`.
- **S-5.9.6-7** **Equal pattern**: matched value as *left*, pattern value as *right*, into the equality
  expression §4.9.19 `[L6619]`; `true` ⇒ match; no applicable overload **or** `false` ⇒ no match, the
  no-overload determination being **compile-time** `[L6620–6622]`.
- **S-5.9.6-8** **Comparison pattern**: matched value as left operand, pattern value as right operand,
  into that operator's comparison expression `[L6623]`; same split with a compile-time no-overload
  determination `[L6625–6626]`.
- **S-5.9.6-9** **`is` pattern**: substitute the matched value into an `is` expression §4.8
  `[L6627]`; the target **must** be a class type, interface, or variant member `[L6629]`;
  class/interface + `Name` ⇒ **immediately assign the narrowed value** `[L6630]`; variant member
  **with** payload + `Name` ⇒ **immediately assign the payload** `[L6631]`; variant member **without**
  payload ⇒ `Name` **forbidden** `[L6632]`; the binding type **may** use §2.5 shorthand `[L6633]`;
  target = the illegal-value literal ⇒ `Name` **forbidden** `[L6634]`.
- **S-5.9.6-10** **Wildcard must always match** `[L6635]`.
- **S-5.9.6-11/12/13** The guard **must be** `boolean`/implicitly convertible `[L6639]` and **must** be
  evaluated after a successful pattern match `[L6640]`; `false` ⇒ the case is not taken `[L6641]`.
- **S-5.9.6-14/15/16** Behaviour: (1) compute the matched expression into `V` `[L6645]`; (2) try cases
  in order — on a pattern hit run the guard if any (false ⇒ next case), execute the case `Block`, and
  if `continue` ran exit the block and continue at (2), otherwise exit the control flow
  `[L6646–L6650]`; (3) no case matched ⇒ exit `[L6651]`.
- **S-5.9.6-17/18** A `continue` targeting this `match` **must** (1) stop the current `Block`, (2)
  continue matching the **next** case `[L6655–6658]`; this is **not** fall-through `[L6660–6662]`.
- **S-5.9.6-19/20** The compiler **must** run an **exhaustiveness check** and **must warn** on failure
  `[L6666–6667]`; there is no `default` arm — exhaustiveness is explicit, e.g. `case wildcard`
  `[L6581–6596, L6698]`.

### 3.16 `break` `[L6767–6787]`
- **S-5.9.7-1..5** `break` **must** appear inside a logical control flow `[L6778]` (else "unexpected
  break" `[L6813–6817]`), and an optional `Name` **must** point to a defined flow `[L6779–6780]`;
  unnamed ⇒ **must exit the innermost naming-capable flow** `[L6784–6786]`, named ⇒ **exit that
  flow** `[L6787]`; the naming-capable set is `while`/`do while`/`for`/`match` `[L6216–6220]`.

### 3.17 `continue` `[L6824–6893]`
- **S-5.9.8-1..7** `continue` **must** appear inside a logical control flow `[L6835]` and an optional
  `Name` **must** point to a defined one `[L6836–6837]`; unnamed ⇒ **must trigger the innermost**
  naming-capable flow `[L6841–6843]`, named ⇒ that flow's `[L6844]`; a flow without special behaviour
  **must** perform the default `[L6845]` — exit the current `Block` and continue the control flow
  `[L6849]`; `match` is special (next case) `[L6655–6658, L6866–6882]`.

### 3.18 Exception model `[L6897–6929]`
- **S-5.10.1-1/2** Exception handling uses `throw` §5.10.3 and `trap` §5.10.4 `[L6901]`; the Built-In
  Library **must define in the global scope** `Exception` (base class `[L6907]`),
  `NotImplementedException` `[L6908]`, `DivideByZeroException` `[L6909]`, `OutOfRangeException`
  `[L6910]`, `AllocateFailedException` `[L6911]`, `DeallocateFailedException` `[L6912]` §15.2
  `[L6903]`.
- **S-5.10.1-3/4/5** A **runtime exception** is an exception value the language automatically produces
  when an operation fails `[L6913]`; an **exception source** is an expression producing one, a `heap`
  definition, or a `dispose` statement `[L6914–L6915]`; runtime exceptions are **only**
  `DivideByZeroException`, `OutOfRangeException`, `AllocateFailedException`,
  `DeallocateFailedException` `[L6919]` — `NotImplementedException` is never auto-produced.
- **S-5.10.1-6/7** A source **outside every `TrapBody`** ⇒ the program **must terminate** `[L6920]`;
  otherwise handling starts at the **innermost containing trap** `[L6921]`: match `catch` in order
  `[L6922]`, execute the hit `[L6923]`, execute that trap's `finally` `[L6924]`; with no hit execute
  `finally` and hand the exception to the next outer containing trap `[L6925]`; with no outer trap the
  program **must terminate** `[L6926]`.
- **S-5.10.1-8/9** The source→`trap` relation **must** be fixed at compile time from **source nesting**
  and **must not** depend on the call stack or inlining `[L6927–6928]`; a runtime exception source
  inside a `catch` or `finally` block ⇒ the program **must terminate immediately** `[L6929]`.

### 3.19 `return` `[L6931–6964]`
- **S-5.10.2-1/2/3** `return` **must appear only in a function body** `[L6942]` (file-level `return`
  mandated error `[L7008–7010]`); the expression type **must be** the return type or implicitly
  convertible `[L6944]`; the expression **may** be omitted when the return type is the empty type
  `[L6945]` (a non-empty type requires one).
- **S-5.10.2-4/5/6** The value **must** become the return value and exit the function `[L6949–6950]`;
  for a reference return type the instance body **must be constructed into caller-provided storage**
  `[L6951]`; if the returned expression is an **identifier whose storage is that body**, the compiler
  **must** construct it there `[L6952]` and **must error** when that cannot be determined `[L6953]`.
- **S-5.10.2-7** The callee does not own that storage: `return MyClass();` never lands in frame
  storage, and `local a = MyClass(); return a;` relies on storage elimination into the caller's
  storage `[L6955–6958]`.
- **S-5.10.2-8/9/10/11** A non-empty return type **must** terminate on **all** paths with `return`
  **or** with a `throw` not intercepted by any `trap` in the function `[L6962]`; an empty return type
  **may** fall off the end `[L6963]`; the compiler **must** decide this **independently of specific
  control-flow structures** `[L6964]`; the `??` return continuation is §4.9.26 `[L6965]`.
  Enumeration → underlying-type implicit conversion is legal here (`return Status.Running;` with
  `cardinal(2)` `[L6979–6983]`).

### 3.20 `throw` `[L7013–7037]`
- **S-5.10.3-1..8** `throw` is a **local** statement that throws a value inside a `trap` or `catch`
  for a `trap` to intercept `[L7015]`, and it **must appear only in a `trap` block or a `catch`
  block** `[L7024]` (`throw 1;` at function level mandated error `[L7076–7078]`); evaluate the
  expression — the result is the **thrown value** `[L7029]`, which **must** be handled by the
  **nearest `trap` able to intercept it** `[L7030]`; with none the program **must terminate**
  `[L7031]`; the value **may be of any type** `[L7035]`, interception **must** match by type
  `[L7036]`, and `throw` is **not** how runtime exceptions are produced `[L7037]`.

### 3.21 `trap` `[L7081–7140]`
- **S-5.10.4-1..4** `trap` is a **local** statement intercepting thrown values from this block and
  inner `trap`s `[L7083]`; form per §2.20 — zero or more `catch`, at most one `finally`, in that order
  `[L7086]`; all bodies are `Block` `[L7088–L7090]`; `CatchCondition = [ Name ":" ] Type |
  "wildcard"` `[L7091–7092]`, and omitting `Name` means **no binding** `[L7104–L7105]`.
- **S-5.10.4-5/6** `trap` **must appear only in a function body** `[L7098]` (top-level `trap` mandated
  error `[L7204–7207]`, but see OQ-6); catch binding types **may** use §2.5 shorthand `[L7108]`.
- **S-5.10.4-7/8/9** Runtime exceptions **must** be handled as thrown values `[L7112]`; a value
  produced in the `TrapBody`, or in an inner `trap` and not intercepted there, that reaches this
  `trap` **must** trigger interception **if the trap has `catch`es** `[L7113]`; interception **must
  terminate the `TrapBody`** and match conditions in order `[L7114]`.
- **S-5.10.4-10/11/12** `wildcard` **always matches** `[L7115]`; otherwise a condition matches iff the
  target type **equals** the thrown value's type **or** is a **base class or interface** of it
  `[L7116]`; `wildcard` **must be last** `[L7117]`.
- **S-5.10.4-13/14** A value thrown **inside a catch must not** be intercepted by the current `trap`
  `[L7118]`; with no matching condition, exit the `trap` statement and continue to an outer `trap` if
  one can intercept, else the program **must terminate** `[L7119]`.
- **S-5.10.4-15** On a match: assign the thrown value to the name if present, execute the catch block,
  exit the `trap` statement `[L7123–7125]`.
- **S-5.10.4-16** `finally` **must** execute when `[L7129–L7133]`: the body/catch exits normally;
  executes `return`; executes `break`/`continue`; or produces a thrown value not intercepted here.
- **S-5.10.4-17/18** `finally` **must not** contain `return`, `break`, `continue` or `throw` `[L7134]`
  (`finally { return; }` mandated error `[L7209–7212]`); on exit the pending action **must** be
  restored `[L7135–L7140]`: intercepted `return`, continuation of a non-intercepted thrown value, a
  catch block's `throw`, `break`, `continue`.

## 4. Exact keyword / operator spellings, and terminators

Keywords (literal terminals of §5; all lower-case except `Construct`/`Destruct`):

| Token | Line | Role |
|---|---|---|
| `local` | 5588 | acquires instance storage in the current scope `[L5585]` |
| `fixed` | 5672 | initialization-only assignment `[L5665]` |
| `heap` | 5759 | acquires instance storage on the heap `[L5756]` |
| `manual` | 5766 | modifier preceding `heap`; exempts from NOLAA `[L5766, L5814]` |
| `linkable` | 5864 | exposes an external symbol `[L5861]` |
| `use` | 5943 | imports members into the current scope `[L5941]` |
| `dispose` | 6112 | destroys heap storage `[L6109]` |
| `if` / `else` | 6251 | conditional / alternative arm |
| `while` | 6325, 6388 | pre-test loop / post-test tail |
| `do` | 6388 | post-test loop head |
| `for` / `in` | 6440 | traversal loop / traversal separator |
| `break` / `continue` | 6772, 6829 | exit flow / continue flow |
| `match` / `case` | 6581, 6583 | pattern dispatch / arm |
| `is` | 6593 | `is` pattern test (cf. `IsExpression` `[L4230]`) |
| `wildcard` | 6594, 7092 | wildcard pattern (and catch-all condition) `[L6594, L7103]` |
| `when` | 6596 | guard |
| `return` / `throw` | 6936, 7018 | function exit / local throw |
| `trap` / `catch` / `finally` | 7086 | interception / arm / finalization |
| `uninitialized` | 5695 | forbidden as constant init; §2.7.12 `[L1267]` |
| `illegal` / `infinite` | 6718 / 4236 | NaN literal §2.7.11 `[L1228, L6634]` / ±infinity `IsTarget` §2.7.10 `[L1188]` |
| `static` / `this` / `Construct` / `Destruct` | 5711 / 5707 / 5922 | `fixed` modifier §9.15 / method marker §8.4 / forbidden `linkable` names |

Operators and punctuation:

| Token | Line | Meaning |
|---|---|---|
| `=` | 5994 | assignment / initializer `[L5590]` |
| `+=` `-=` `*=` `/=` `%=` `&=` `\|=` `^=` `<<=` `>>=` | 5996 | the exactly-10 compound assignment operators |
| `..` | 6444 | left-closed right-closed range `[L6501]` |
| `..!` | 6444 | left-closed right-open range; single token by maximal munch `[L6458, L6502]` |
| `&` | 6440 | reference marker on the `for` iteration variable `[L6452]` |
| `:` | 6210 | logical control flow name prefix (also type separator) |
| `,` | 5588, 5943 | `LocalItem` / `use` target separator |
| `!=` `>` `<` `>=` `<=` | 6592 | the exactly-5 comparison-pattern operators |
| `{` `}` / `;` | 5535 / 5954 | block delimiters (also terminators via §2.8) / statement terminator |
| `(` `)` / `,` | 5759, 5864 / 5588 | allocator expression, symbol literal, parenthesized conditions / item separator |

Terminators (from the examples; §2.8 semantics `[L1313–1335]`): `local`, `heap`, `linkable`, `use`,
assignment, `dispose`, `do…while`, `break`, `continue`, `return`, `throw`, `fixed … = expr` take `;`;
`if`, `while`, `for`, `match`, `trap`, `fixed … Block` and bare `Block` are terminated by `}`
`[L5570, L5726, L5733]`.

## 5. Example inventory (`novasharp` blocks)

| # | Lines | § | Content |
|---|---|---|---|
| 1 | 5562–5581 | 5.1 | `Main` block with `local a`; two commented-out illegal unclosed blocks. |
| 2 | 5635–5655 | 5.2 | Legal typed/inferred/uninitialized `local`; illegal `local bad;`; illegal `local` in a `structure` body. |
| 3 | 5659–5661 | 5.2 | Three `LocalItem`s in one statement: `local a = 1, b = () => { return; }, c: boolean;`. |
| 4 | 5715–5752 | 5.3 | Hex/inferred `fixed`, generic `Echo<T>`, `fixed Desugar() = () => { }`, uninitialized interface constant; illegal missing init and initialized interface constant. |
| 5 | 5821–5857 | 5.4 | Default/explicit allocator, `manual heap`, value-type heap, field and heap-storage overwrite; illegal `manual` after `heap`. |
| 6 | 5930–5936 | 5.5 | Four `linkable` declarations incl. `public linkable("Total") ExternalValue`. |
| 7 | 5964–5987 | 5.6 | Two modules; `use MyModule, OtherModule;` at file level and `use MyModule;` in a block. |
| 8 | 6039–6105 | 5.7 | `Counter` (array field, accessor, indexer get/set); assignments to variable/compound/element/field/accessor/indexer/heap field; illegal assignment to `fixed`. |
| 9 | 6158–6201 | 5.8 | `Token`; escaping `Make()`; dispose auto heap, manual heap, and a `local` referencing a heap unit; illegal dispose of a scope instance; use-after-dispose. |
| 10 | 6282–6318 | 5.9.2 | `if`/`else if`/`else`; illegal non-`boolean` condition, condition reading a block-local, string condition; illegal `if` outside a function. |
| 11 | 6351–6381 | 5.9.3 | `while true`, named `while false : Loop`; illegal non-`boolean` condition, block-local read; illegal outside a function. |
| 12 | 6414–6433 | 5.9.4 | `do { } while true;`, `do { local x = 1; } while x == 1;`, nested block; illegal outside a function. |
| 13 | 6525–6574 | 5.9.5 | Array copy vs `for &ref`; `0..5`, `0..!5`, `5..!0`, `{[0..!5]}`, `0..2..10`, `10..-2..0`; illegal range type mismatch, illegal post-loop use of the iteration variable, illegal outside a function. |
| 14 | 6671–6765 | 5.9.6 | `Animal`/`Cat`/`Option`; equal, narrowing `is cat:Cat`, payload-binding `is value:Option.Some`, `is illegal`, `> 3`/`< 0`, guard `when v > 3`, statically-false `is cat:Cat`; illegal `is i:Option`, illegal binding on a payload-less member, illegal outside a function. |
| 15 | 6791–6822 | 5.9.7 | `break Outer`; `break;` in `if` inside `for`; illegal bare `break` in an `if` with no enclosing flow. |
| 16 | 6853–6893 | 5.9.8 | `continue Outer` from an inner `while`; `continue;` skipping to the next `match` case (no fall-through); illegal `continue` in `if`. |
| 17 | 6969–7011 | 5.10.2 | `return 14`, enum→`cardinal(2)`, `return;`, generic `Echo<T>`; illegal missing return path (`if (flag)`), illegal `return` outside a function. |
| 18 | 7041–7079 | 5.10.3 | `throw` in a trap body caught by `OutOfRangeException`; nested trap whose catch throws, caught by outer `Exception`; illegal `throw 1;`. |
| 19 | 7144–7213 | 5.10.4 | `Handle`, `Nested` (base-class catch), `WithFinally` (finally before return), top-level `catch wildcard`; illegal top-level `trap`, illegal `return` in `finally`. |

## 6. Derived facts (deductions, not new rules)

- **D-1/D-2** An `if` can never be a `break`/`continue` target — names exist only on
  `while`/`do while`/`for`/`match` `[L6216–6220]` and unlabelled forms pick the innermost
  naming-capable flow `[L6784–6786, L6841–6843]` (confirmed `[L6804–6811, L6857–6864]`); `match` is
  therefore a target, its special `continue` defined `[L6655–6658]` and its `break` not (OQ-12).
- **D-3** With `step == 0` the traversal condition is false on both sides (`step > 0` and `step < 0`
  both fail), so the body never executes; no step-0 rule exists, this only follows from
  `[L6518–6519]`.
- **D-4/D-5** `do { } while c;`'s condition is the only one that may read body locals
  `[L6397–L6398]` vs `[L6260, L6334]`, so the body's scope stays open at the latch; a `trap` with zero
  `catch`es is grammatical `[L7086]`, and since interception needs a `catch` `[L7113]` while `finally`
  still runs `[L7133]`, `trap { … } finally { … }` is a pure finalizer.
- **D-6** `throw` is lexically confined to `trap`/`catch` `[L7024]` and interception is resolved by
  source nesting, not the call stack `[L6927–6928]`, so a `throw` can never escape the function that
  lexically contains it; with no matching catch the program terminates `[L7031, L7119]`. **D-7** The
  `FunctionShorthand` alternative of `linkable` has no `=`/`Block`, so an externally visible function
  **with a body** must use the second alternative, while a body-less one must satisfy `[L5893]`
  (`:` present) unless written in shorthand form.

## 7. Open questions

Each item: the ambiguity, the spec lines, and a **proposed** interpretation (proposal ≠ spec).

- **OQ-1 Bare-guard case is grammatical but undefined.** `[L6583]` admits `case when x > 0 { }` (no
  pattern) while the behaviour assumes a pattern test `[L6646]`. *Proposal:* treat it as
  `wildcard`+guard; reject under a strict flag until clarified.
- **OQ-2 `for` requires `":" Type`, yet the text invokes §2.5 shorthand.** `[L6440]` vs `[L6464]`,
  and §2.5 forbids shorthand-driven inference `[L462–463]`. *Proposal:* `:` mandatory; shorthand only
  reshapes `Name ":" Type`; `for i in 0..5` is a syntax error.
- **OQ-3 Range step with `..!`.** `[L6444]` derives `start..step..!end` and `start..!end`, but the
  prose `[L6459–6460]` and all examples only show the unmixed forms. *Proposal:* accept it; step =
  middle `Expression` if present, end = final one; `..!` a single token `[L6458]`.
- **OQ-4 Default step is a run-time decision; step 0 unmentioned.** `[L6503–6505]` compares values at
  run time and `5..!0` is shown descending `[L6545]`; no step-0 rule exists. *Proposal:* compute
  start/end (and the derived sign) once in the preheader; step 0 ⇒ zero iterations (D-3).
- **OQ-5 "Real-time" step/end vs. a target fixed at entry.** `[L6520–6521]` vs `[L6547–6548]`.
  *Proposal:* evaluate `TraverseTarget` once at entry, re-evaluate only the `Range`'s step/end
  sub-expressions, and let the optimizer hoist (explicitly permitted).
- **OQ-6 Top-level `trap` example contradicts the placement rule.** `[L7098]` forbids it and
  `[L7204–7207]` marks it illegal, yet `[L7195–7202]` labels a top-level `trap … catch wildcard` as
  legal. *Proposal:* read 7195–7202 as an abbreviated in-function illustration; keep `[L7098]`.
- **OQ-7 Runtime exceptions never cross a function boundary.** `[L6927–6928]` makes source→trap purely
  lexical and `[L6920]` terminates the program when the source is in no `TrapBody`, so a
  divide-by-zero/out-of-range in a callee cannot be caught by the caller — it aborts. **Most
  consequential ambiguity for codegen.** *Proposal:* implement the non-unwinding contract (source
  site → lexically innermost trap, else terminal call); confirm with the spec owner.
- **OQ-8 `[L6929]` vs. thrown values from `catch`/`finally`.** `[L6929]` aborts when an exception
  source sits in `catch`/`finally`, while `[L7030, L7118–L7119]` let a catch's `throw` propagate and
  `[L7133, L7135–L7140]` require `finally` to run first. *Proposal:* `[L6929]` covers **runtime
  exceptions only** `[L6913–L6914]`, so explicit `throw` propagates; residual: a failing
  `heap`/`dispose`/index expression inside a catch block aborts with no guard.
- **OQ-9 `throw` placement across nested function literals.** `[L7024]` is silent on a function
  literal lexically inside a `TrapBody`. *Proposal:* the restriction is per function body, so such a
  `throw` is an error; confirm.
- **OQ-10 Extent of the `finally` content ban.** `[L7134]` does not say whether it is transitive over
  nested blocks, nor whether `break` targeting a loop *inside* the `finally` is banned. *Proposal:*
  forbid those statements transitively when they would leave the `finally`; allow `break`/`continue`
  whose target lies wholly inside it.
- **OQ-11 `uninitialized` as a `local` initializer.** `[L5695]` bans it only for `fixed`; §12.1
  requires it for storage readable before assignment `[L9453]`; §5.2 never mentions it
  `[L5611–5618]`, and `local x = uninitialized` has no inferable type `[L5618]`. *Proposal:* allow
  `local x: T = uninitialized`, reject the inferred form, apply §12.1's warnings `[L9457–L9459]`.
- **OQ-12 `break` inside `match` is unspecified.** `match` is naming-capable `[L6216–6220]` and break
  exits the innermost such flow `[L6784–6786]`, but §5.9.6 defines only `continue` `[L6655–6658]` and
  the normal exit `[L6650]`. *Proposal:* `break` exits the `match`; document as derived, confirm.
- **OQ-13 Where a flow name is *declared* for uniqueness.** `[L6233]` forbids duplicates "in the same
  scope" while `[L6238–6239]` scopes the name to the flow's own blocks; read strictly, two sequential
  `:L` loops in one block conflict. *Proposal:* a synthetic scope enclosing the flow's `Block` but not
  its siblings, so sequential `:L` is legal; confirm (changes diagnostics).
- **OQ-14 `&` on non-array traversal targets.** `[L6452–6454]` defines the marker generally but
  reference semantics exist only for arrays `[L6478–L6479]`. *Proposal:* `&` is an error unless the
  target resolves to an array (S-5.9.5-7 cases 1–2).
- **OQ-15 Array element type vs. iteration variable type.** `[L6487]` constrains only the iterator
  payload; arrays say only "copy of the element" `[L6479]`. *Proposal:* require element type →
  iteration type to be implicitly convertible, mirroring the payload rule.
- **OQ-16 `linkable` alternative overlap / bodies with shorthand.** Both alternatives `[L5864–5865]`
  derive a body-less declaration and only the second carries `=`/`Block`, so (a) ownership of
  `linkable f(x: integer(4)): integer(4);` is ambiguous and (b) an inline-bodied `linkable` function
  constant cannot be written in shorthand. *Proposal:* parse `FunctionShorthand` when `(` follows the
  name, treat both as semantically identical, require `Name: function(...) T Block` for bodies.
- **OQ-17 `dispose` excludes index access.** `[L6112]` allows only `Identifier` and
  `PointerMemberAccessExpression`, whereas assignment allows `IndexAccessExpression`
  `[L5999–L6000]`, so an element of an array of heap references cannot be disposed. *Proposal:* keep
  the grammar restriction (`dispose arr[i]` = syntax error); revisit if intended.
- **OQ-18 Scope of the `case is name:T` binding.** `[L6630–6631]` bind immediately, `[L6640]`
  evaluates the guard after the match, `[L6736]` reads the binding in the guard; the extent after the
  case block and redeclaration across sibling cases are unstated. *Proposal:* scope = guard + case
  `Block`; names may repeat across sibling cases; nested `match` may shadow.
- **OQ-19 `Name` on special-float `is` targets.** `[L6634]` bans binding only for the illegal-value
  literal, but `IsTarget` also admits `[ "-" ] "infinite"` `[L4232–4236]`, where binding is
  meaningless and §4.8 requires a float operand `[L4253]`. *Proposal:* ban `Name` for all
  `SpecialFloatTarget`s with the `[L6634]` diagnostic.
- **OQ-20 `throw` as an all-paths terminator.** `[L6962]` accepts an uncaught `throw` as terminating,
  yet such a path always aborts `[L7031]`. *Proposal:* accept literally — the checker only needs "no
  fall-through path reaches the closing brace".
- **OQ-21 Compiler determinability of `dispose` on reference-type `local`s.** `[L6129]` requires the
  compiler to decide "points to a heap unit", and `[L6186–6187]` disposes a `local` bound from a
  function returning a reference; if that object were scope storage the dispose would be illegal, so
  the example relies on §12.1's escape rule `[L9461–L9469]`. *Proposal:* track storage origin (heap
  unit / scope storage / unknown), unknown ⇒ error; a callee-returned reference instance is caller
  storage by `[L6951–L6952]` and therefore **not** disposable.
- **OQ-22 "Target evaluated once" for accessor targets.** `counter.Value += 1` needs a get/set pair
  `[L6012]`, and `[L6029–L6030, L6035]` forbid a naive expansion; a write-only setter makes the
  operation inexpressible. *Proposal:* require a readable destination and diagnose instead of
  synthesizing `a = a op b`.

## 8. LLVM 18 codegen implications

Derived from §3 only; nothing here adds language rules. OQs are cited where a choice depends on one.

### 8.1 Allocation classes
| Source form | Storage | Lowering shape (prose) | Rule |
|---|---|---|---|
| `local` in function body | stack, scope duration | `alloca` in the entry block (or a `llvm.lifetime`-scoped region), one slot per `LocalItem`, released at scope exit | `[L5627–5629, L9473]` |
| `local` in module/file body | global data area | module-level `global` (zero-initialized if no `=`) | `[L5630]` |
| `fixed … = expr` | read-only static | `global constant` in a read-only section (§14.2), never a mutable slot | `[L5694, L5696]` |
| `fixed` interface member | none | no storage, no initializer — contract only | `[L5692]` |
| `fixed` function constant | code | `define`, linkage per §9.15 (`internal` by default) | `[L5702–5711]` |
| `heap` / `manual heap` | heap unit | `alloca ptr` binding + call into `IAllocator.Allocate`; pointer in the slot; NOLAA suppressed for `manual` only | `[L5759, L5796, L5814, L9528]` |
| `linkable` | external symbol | `@sym = global …` with external linkage (or `declare` when no initializer); never `internal`, never dropped; name = literal content or `Name`, unmangled | `[L5900–L5908, L5892]` |
| `linkable` function constant | function | `define` under the exposed symbol name; optional `Block` body | `[L5894, L5906]` |

Notes: reference-type heaps "store a reference to that storage" `[L9431]`, value-type heaps hold the
value inside the unit (`number = 43` `[L5849]` writes through the recorded pointer); the **unit must
record the allocator** `[L5795]`, so a runtime side table (unit → {allocator, size, destructor}) or a
hidden header word is mandatory for `dispose` `[L6136]`; module/file-scope accessibility maps to
linkage (public ⇒ external) but the range is silent (§9.1 `[L5609]`); `use` has no codegen effect.

### 8.2 Initialization
- `local x: T;` / `heap x: T;` write nothing `[L5617, L5805]`; reads before assignment are §12.1
  `[L9450–9459]` — leave the slot unwritten, never constant-fold or speculate `[L9456]`, and emit the
  mandated warnings for function/pointer/reference typed storage `[L9457–L9459]`.
- Constructor initializers (`[L5834, L6079]`) **must** construct directly into the destination
  `[L6023, L9438]` — no temporary; this is the same rule that drives `sret` for returns.
- `heap` order: evaluate the allocator expression **once** (or load global `DefaultAllocator`
  `[L5790–L5794]`) → `Allocate` → branch to the `AllocateFailedException` path **before** binding the
  name and **before** the initializer `[L5797–L5798]` → bind slot → initializer → store.
- `fixed` outside interfaces **must** be initialized `[L5693]` and `uninitialized` is banned
  `[L5695]`: emit constant data, no runtime store.

### 8.3 Assignment
- `SetStatement`: resolve the destination **once** `[L6034]`, evaluate the RHS, store `[L6020]` —
  value-type destination stores the instance body (`memcpy` for aggregates) `[L6021]`, reference-type
  stores the pointer `[L6022]`, a construct-expression RHS constructs in place `[L6023]`;
  field/accessor/indexer destinations lower to their `set` accessor or indexer `set` call
  `[L6011–L6014]` (`[L6084–L6088]`).
- `CompoundSetStatement`: **must not** be the `a = a op b` expansion `[L6030]`. Order: (1) resolve
  destination, (2) evaluate RHS, (3) load current value, (4) call the target type's binary operator
  §4.9.2, (5) store — the load is **after** the RHS `[L6035]`, destination resolution is **before**
  it `[L6034]`, and all target subexpressions are evaluated exactly once `[L6029]` (an indexer target
  calls `get` once and `set` once). Write-only accessor destinations are inexpressible (OQ-22).

### 8.4 Control flow
- `if`: `br` chain — test block per condition, block per arm, join at exit; `else if` is linear
  `[L6269–6278]`; conditions are ordinary expressions (parentheses optional `[L6265]`, but
  `if (flag)` `[L7001]` must parse).
- `while`: preheader → header (condition) → body → latch → header / exit; the condition cannot see
  body locals `[L6334]`, so no dominance problem. `do while`: body → latch (condition) → body/exit,
  and the condition **is** in the body's scope `[L6398]` — allocate body slots in the entry block (or
  keep the lifetime region open through the latch) or verification will fail.
- `for` over an array `[L6478–L6479]`: preheader evaluates the target once (OQ-5), takes pointer +
  length, uses an index induction variable and a GEP element address — with `&` use the address
  (write-through, `ref = 0` `[L6533–6536]`), otherwise load a copy.
- `for` over `IIterator` `[L6483–L6487]`: keep the iterator in a slot; body = `Next()` → branch on
  `Value`/`Null`; `Value` ⇒ store payload (implicit conversion `[L6487]`) → user block → loop; `Null`
  ⇒ exit. `IIterable` `[L6493]`: load the `Iterator` accessor once, then identical.
- `for` over a range `[L6509–6521]`: preheader computes start/end and (if omitted) a run-time `select`
  of ±1 from `start <= end` (OQ-4); header tests the two-sided condition `[L6518–L6519]`; latch adds
  the step. Do **not** treat step/end as loop-invariant in the front end `[L6520–L6521]` — keep them
  as written and let LLVM hoist.
- `break`/`continue`: one exit block and one latch block per naming-capable flow; resolve names
  innermost-first `[L6243]` against a compile-time label stack, unnamed forms using the innermost
  entry (D-1); the iteration variable must be invisible after the loop `[L6565–L6568]`. A `trap` with
  `finally` between the jump and its target forces the §8.6 pending-action path `[L7132, L7135–L7140]`.

### 8.5 `match`
- Lower to a linear case chain: entry computes `V` once into a slot `[L6645]`; `case_i` tests pattern +
  guard, success → `block_i`, failure → `case_{i+1}`, last failure → match exit `[L6646–L6651]`.
- Patterns: `EqualPattern` ⇒ the §4.9.19 equality overload with `(V, pattern_value)`, an inapplicable
  overload being a **compile-time** diagnostic `[L6622]`; `ComparisonPattern` ⇒ the operator for
  exactly `!=`, `>`, `<`, `>=`, `<=` `[L6592]` (never `==`); `IsPattern` ⇒ the §4.8 test (class ⇒
  type-id equality/base-chain walk, interface ⇒ interface-table lookup, variant member ⇒ discriminant
  compare, special-float targets ⇒ FP NaN/±inf test `[L4255–4261]`), binding immediately on success
  with a `Name` (narrowed value `[L6630]` / extracted payload `[L6631]`; payload-less members reject
  `Name` `[L6632]`); `WildcardPattern` ⇒ unconditional `[L6635]`; `Guard` ⇒ evaluated inside the
  success path so bindings are visible `[L6640, L6736]`, `false` ⇒ next case `[L6641]`.
- `continue` in a case branches to `case_{i+1}` `[L6649, L6655–6658]`, so each case needs both a
  next-case and a match-exit edge; `break` branches to the match exit (OQ-12); normal completion
  branches to the exit `[L6650]`, with no fall-through `[L6662]`. Exhaustiveness `[L6666–L6667]` is a
  **Warning only** — never synthesize a runtime fallback. Register the flow's `:Name`.

### 8.6 Exception machinery (`throw` / `trap` / runtime exceptions)
1. Catch targets are fixed **lexically** at compile time `[L6927–6928]`: do **not** discover them with
   stack unwinding (`invoke`/`landingpad`); a source site's catch set is its lexical `trap` chain.
2. Lower each `trap` as a region: body region, exception slot (`alloca`, type-erased to pointer + type
   id since a thrown value may be any type `[L7035]`), dispatcher, one test block per `catch`, catch
   blocks, optional `finally`, exit.
3. `throw` `[L7018–L7030]`: store value + type id into the **innermost enclosing `trap` region's**
   slot and branch to its dispatcher, skipping the rest of the current body/catch.
4. Dispatcher `[L7114–L7119]`: test catches in source order, `wildcard` unconditional (**must** be
   last `[L7117]`, so a linear chain with an unconditional tail is exact); otherwise compare type ids
   and walk base classes/interface tables — equality, base, or interface `[L7116]`.
5. Matching catch `[L7123–L7125]`: if named, store the pointer-adjusted upcast into the binding slot;
   run the block; go to the `trap` exit. A `throw` **inside** a catch routes to the **outer** region
   `[L7118]`.
6. No catch matched `[L7119]`: run `finally` `[L7133]`, propagate to the next outer **lexical**
   `trap`; if none, the program **must terminate** `[L6920, L6926, L7031]` — call a terminal runtime
   routine (abort/trap), not `unreachable` alone, so diagnostics survive.
7. `finally` `[L7129–L7140]`: pending-action slot + switch after the block, actions `normal`,
   `return(value)`, `throw(value,type)`, `break(target)`, `continue(target)`; the slot is written by
   the pre-`finally` jump and read by the post-`finally` dispatch, because `finally` may not transfer
   control itself `[L7134]`. A reference-type `return` stores the caller storage address (§8.7).
8. Runtime exceptions (`AllocateFailedException` `[L5797]`, `DeallocateFailedException` `[L6137]`,
   `DivideByZeroException`, `OutOfRangeException` `[L6919]`): each source site knows its lexical trap
   chain, so lower exactly like a `throw` from that site — **within the same function**. A runtime
   failure in a **callee** cannot be caught by the caller (OQ-7); if that changes, this is the only
   place needing real unwinding, so keep the region model behind a small abstraction.
9. Runtime exception sources inside `catch`/`finally` terminate the program immediately `[L6929]`
   (OQ-8): emit a direct terminal call, no dispatcher edge. Under the lexical model no
   `setjmp`/`longjmp`, personality function or `llvm.eh.*` intrinsic is needed; `trap` bodies are
   ordinary blocks.

### 8.7 `return`
- Compute and `ret` the value; aggregates go through `sret` per the target ABI. Reference-type
  returns `[L6951–L6952]` construct into **caller-provided storage** (explicit hidden pointer):
  `return MyClass();` constructs there directly, and `local a = MyClass(); return a;` requires
  **storage elimination** (allocate `a` at that address, or promote/memcpy with the local removed).
  The spec requires a compiler **error** when that cannot be determined `[L6953]` — diagnose rather
  than copying and returning a dangling reference.
- All-paths termination `[L6962–L6964]`: a CFG reachability check ("can the closing brace be
  reached"), independent of source shapes; a path ending in an uncaught `throw` counts `[L6962]`
  (OQ-20); empty-return functions may fall through `[L6963]` (`ret void`).
- `return` inside a `trap` body/catch with a `finally` in between goes through §8.6 item 7
  `[L7131–L7140]`.

### 8.8 NOLAA and `dispose`
- NOLAA is whole-program `[L9526–L9527]`; its only code effect is inserting explicit `dispose` at safe
  points of non-`manual` units `[L9514, L9534]`, conservatively `[L9535–L9539]`. Model `dispose` as a
  first-class call sequence so inserted and source-written ones lower identically.
- `dispose x` `[L6134–L6139]`: destructor for `x`'s dynamic type `[L6133]`; if incomplete, skip
  reclaim `[L6135]`; then `IAllocator.Deallocate(unit, size)` on the recorded allocator `[L6136]`;
  `Alternative` ⇒ `DeallocateFailedException` `[L6137]`; only the pointed-to unit, never transitive
  `[L6138–L6139]`.
- Double-dispose/use-after-dispose `[L6150–L6152]`, Early-Dispose `[L6146]` and Never-Disposed
  `[L9513]` are warnings only. A unit can outlive its binding slot `[L5810]`, so its header must not
  be tied to the stack slot.

### 8.9 Diagnostics required by this range
Compile errors: missing type+initializer `[L5605, L5774]`; constant uninitialized `[L5693]`; initialized
interface constant `[L5692]`; `uninitialized` constant init `[L5695]`; generics on a non-function
constant `[L5702]`; `linkable`+`fixed` `[L5884]`; non-accessibility `linkable` modifier `[L5888]`;
`linkable` in a function body or direct type-body member `[L5921]`; `linkable` named
`Construct`/`Destruct` `[L5922]`; assignment outside a function body `[L6006, L6008]`; assignment to a
constant `[L6099–L6101]`; non-assignable target `[L6011–L6012]`; `dispose` outside a function body or
on a non-heap/constant target `[L6118, L6121–L6129]`; `if`/`while`/`do while`/`for`/`match`/`trap`/
`return`/`throw` outside a function body `[L6257, L6331, L6394, L6450, L6602, L7098, L6942, L7024]`;
non-`boolean` conditions `[L6259, L6333, L6396, L6639]`; `if`/`while` condition reading a block-local
`[L6260, L6334]`; `break`/`continue` without a target `[L6778, L6835]`; unknown label
`[L6780, L6837]`; `throw` outside trap/catch `[L7024]`; non-last `wildcard` `[L7117]`;
`return`/`break`/`continue`/`throw` in `finally` `[L7134]`; missing return path `[L6962]`; wrong-kind
`is` target `[L6629]`; `Name` on a payload-less variant member or on `illegal` `[L6632, L6634]`; range
type mismatch `[L6561–L6563]`; `&` over a non-array (OQ-14); duplicate flow name `[L6233]`;
undeterminable storage elimination `[L6953]`.

Warnings only: non-exhaustive `match` `[L6666–L6667]`; Early-Dispose `[L6146]`; Double-Dispose
`[L6152]`; Never-Disposed `[L9513]`; undecidable dangerous dispose `[L6153–L6154]`; uninitialized
function/pointer/reference use `[L9457–L9459]`.

## 9. Coverage checklist

- [x] Section map (§1); 20/20 `ebnf` blocks verbatim and in order (§2); exact keyword/operator
      spellings and terminator inventory (§4); 19/19 `novasharp` examples (§5).
- [x] Semantic rules tagged with line ranges for every declaration form, assignment, dispose, all
      logical control flow, exception handling, return/throw/trap and illegal cases (§3), with
      deductions separated from normative rules (§6) and 22 open questions with proposals (§7).
- [x] LLVM 18 codegen implications: allocation, initialization, assignment, control flow, match,
      exceptions, trap/finally, return/sret, NOLAA/dispose, diagnostics (§8).
- [x] Only this file was written; no implementation code.
