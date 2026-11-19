# NovaSharp 表达式（§4 Expressions） — Implementation Note

**Source:** `docs/grammar/Draft 1.1.md`, §4 表达式, lines **3540–5443** (§5 语句 begins at L5444).
**Method:** whole range read; every ```ebnf block copied verbatim below; every ```novasharp example indexed.
**Conventions:** `Lnnnn` = 1-based line in `Draft 1.1.md`. `MUST` = spec **必须**, `MUST NOT` = **不得**, `FORBIDDEN` = **禁止**. Kinds in `code` are exactly the spellings the spec uses (`Cast`, `is`, `infinite`, `illegal`, `implicitable`, `operator ??`, `Result<TValue; TAlternative>`, …). Anything not stated in the range is either cross-referenced (§n) or listed as an **Open Question (OQ-n)** in §7; no rule below is invented.

---

## 1. Section map with line anchors

| § | Title (spec) | Lines | Normative payload |
|---|---|---|---|
| 4 | 表达式 | 3540–3629 | `Expression` sum type (34 alternatives); link table L3583–3617; **evaluation order** L3623–3628 |
| 4.1 | 值表达式 | 3630–3659 | `Identifier` / `Literal` / access forms; identifier MUST be defined & accessible |
| 4.2 | 括号表达式 | 3661–3676 | `( Expression )`; precedence override |
| 4.3 | 类型转换表达式 | 3678–3750 | `(Expression as Type)`; conversion-operator priority; no implicit except `implicitable` |
| 4.4 | 访问表达式 | 3752–3772 | `.` and `->` declared; 4 alternatives |
| 4.4.1 | 成员访问表达式 | 3773–3818 | source kinds; member resolution order; static/payload-less variant rules |
| 4.4.2 | 指针成员访问表达式 | 3819–3852 | deref-then-access; typed pointee required; assignment overwrites storage |
| 4.4.3 | 索引访问表达式 | 3853–3897 | array vs index-accessor; return types; assignment; bounds check + `OutOfRangeException` |
| 4.5 | 函数调用表达式 | 3898–4010 | callee must be function type; overload resolution; argument forms |
| 4.6 | 构造表达式 | 4012–4108 | class/structure only; matching ctor; **in-place construction** on RHS |
| 4.7 | 成员构造表达式 | 4110–4223 | variant/union only; payload state; tag/union lowering |
| 4.8 | is 表达式 | 4225–4333 | class/interface/variant-state/NaN/±infinity tests; boolean |
| 4.9 | 运算符 | 4335–4505 | `OperatorDefinition` grammar; unary/binary shape; indexer; conversion operator; `??` |
| 4.9.1 | 运算符优先级 | 4507–4586 | 16-level precedence table; associativity; homographic `& * ^ -` |
| 4.9.2 | 运算符重载 | 4588–4696 | 12 overloadable symbols; derived `!= <= >=`; builtin vs user overloads |
| 4.9.3 | 取负表达式 | 4698–4742 | unary `-` |
| 4.9.4 | 逻辑非表达式 | 4744–4762 | `!` |
| 4.9.5 | 按位取反表达式 | 4764–4785 | unary `^` |
| 4.9.6 | 引用表达式 | 4787–4840 | `&`, addressability definition |
| 4.9.7 | 解引用表达式 | 4842–4860 | unary `*` |
| 4.9.8 | 乘法表达式 | 4862–4882 | `*` |
| 4.9.9 | 除法表达式 | 4884–4904 | `/` |
| 4.9.10 | 求余表达式 | 4906–4926 | `%` |
| 4.9.11 | 加法表达式 | 4928–4948 | `+` |
| 4.9.12 | 减法表达式 | 4950–4970 | `-` (binary) |
| 4.9.13 | 逻辑左移表达式 | 4972–4996 | `<<` |
| 4.9.14 | 逻辑右移表达式 | 4998–5022 | `>>` |
| 4.9.15 | 小于表达式 | 5024–5044 | `<` |
| 4.9.16 | 大于表达式 | 5046–5066 | `>` |
| 4.9.17 | 小于等于表达式 | 5068–5091 | `<=` → `<` ‖ `==` |
| 4.9.18 | 大于等于表达式 | 5093–5116 | `>=` → `>` ‖ `==` |
| 4.9.19 | 相等表达式 | 5118–5138 | `==` |
| 4.9.20 | 不等表达式 | 5140–5161 | `!=` → `!(==)` |
| 4.9.21 | 按位与表达式 | 5163–5185 | `&` (binary) |
| 4.9.22 | 按位异或表达式 | 5187–5209 | `^` (binary) |
| 4.9.23 | 按位或表达式 | 5211–5233 | `\|` |
| 4.9.24 | 逻辑和表达式 | 5235–5253 | `&&`, short-circuit |
| 4.9.25 | 逻辑或表达式 | 5255–5273 | `\|\|`, short-circuit |
| 4.9.26 | 空值合并表达式 | 5275–5379 | `??`, `throw`/`return` continuations, `Result`/`Optional` |
| 4.9.27 | 条件表达式 | 5381–5442 | `?:` |

Boundary: §5 语句 starts L5444; `ExpressionStatement = FunctionCallExpression` is L5483 (outside range, relevant to call lowering only).

---

## 2. Consolidated EBNF (verbatim, in spec order)

> Every fenced `ebnf` block from L3540–L5443, unchanged, each under the heading of its section. No productions added, removed, or reflowed.

### 2.1 §4 表达式 — `Expression` (L3544–3579)

```ebnf
Expression = ValueExpression
           | ParenthesisExpression
           | TypeConversionExpression
           | AddExpression
           | MinusExpression
           | MultiplyExpression
           | DivideExpression
           | RemainderExpression
           | LogicalNotExpression
           | LogicalAndExpression
           | LogicalOrExpression
           | EqualExpression
           | NotEqualExpression
           | GreaterExpression
           | LessExpression
           | GreaterEqualExpression
           | LessEqualExpression
           | BitwiseNotExpression
           | BitwiseAndExpression
           | BitwiseOrExpression
           | BitwiseXorExpression
           | LogicalRightShiftExpression
           | LogicalLeftShiftExpression
           | ReferenceExpression
           | DereferenceExpression
           | NegateExpression
           | FunctionCallExpression
           | ConstructionExpression
           | MemberConstructionExpression
           | IsExpression
           | AccessExpression
           | ConditionalExpression
           | NullCoalescingExpression
           | CompileTimeExpression .
```

### 2.2 §4.1 值表达式 — `ValueExpression` (L3634–3641)

```ebnf
ValueExpression = Identifier
                | Literal
                | MemberAccessExpression
                | PointerMemberAccessExpression
                | MonomorphizationExpression
                | IndexAccessExpression .
```

### 2.3 §4.2 括号表达式 — `ParenthesisExpression` (L3665–3667)

```ebnf
ParenthesisExpression = "(" Expression ")" .
```

### 2.4 §4.3 类型转换表达式 — `TypeConversionExpression` (L3682–3684)

```ebnf
TypeConversionExpression = "Cast" "<" Type ">" "(" Expression ")" .
```

### 2.5 §4.4 访问表达式 — `AccessExpression` (L3756–3761)

```ebnf
AccessExpression = MemberAccessExpression
                 | PointerMemberAccessExpression
                 | MonomorphizationExpression
                 | IndexAccessExpression .
```

### 2.6 §4.4.1 成员访问表达式 — `MemberAccessExpression` (L3777–3779)

```ebnf
MemberAccessExpression = ( Identifier | Expression ) "." Name .
```

### 2.7 §4.4.2 指针成员访问表达式 — `PointerMemberAccessExpression` (L3823–3825)

```ebnf
PointerMemberAccessExpression = ( Identifier | Expression ) "->" Name .
```

### 2.8 §4.4.3 索引访问表达式 — `IndexAccessExpression` (L3857–3859)

```ebnf
IndexAccessExpression = ( Identifier | Expression ) "[" Expression "]" .
```

### 2.9 §4.5 函数调用表达式 — `FunctionCallExpression` (L3902–3904)

```ebnf
FunctionCallExpression = Expression "(" Arguments ")" .
```

### 2.10 §4.6 构造表达式 — `ConstructionExpression`, `ConstructedTypeName` (L4016–4020)

```ebnf
ConstructionExpression = ConstructedTypeName "(" Arguments ")" .

ConstructedTypeName = Type | MonomorphizationExpression .
```

### 2.11 §4.7 成员构造表达式 — `MemberConstructionExpression` (L4114–4116)

```ebnf
MemberConstructionExpression = ConstructedTypeName "." Name "(" Expression ")" .
```

### 2.12 §4.8 is 表达式 — `IsExpression`, `IsTarget`, `SpecialFloatTarget`, `InfinityTarget` (L4229–4237)

```ebnf
IsExpression = Expression "is" IsTarget .

IsTarget = Identifier | SpecialFloatTarget .

SpecialFloatTarget = IllegalLiteral | InfinityTarget .

InfinityTarget = [ "-" ] "infinite" .
```

### 2.13 §4.9 运算符 — `OperatorDefinition` … `IndexerNotation` (L4339–4351)

```ebnf
OperatorDefinition = { Modifier } "operator" OperatorHead [ "=" Expression | Block | AccessorMembers ] .

OperatorHead      = OperatorShorthand | OperatorNotation [ ":" Type ] .

OperatorShorthand = OperatorSymbol "(" [ Parameters ] ")" [ ":" Type ] .

OperatorNotation  = OperatorSymbol | IndexerNotation | ConstructedTypeName .

OperatorSymbol = "+" | "-" | "*" | "/" | "%" | "==" | ">" | "<" | "??" .

IndexerNotation = "[" Parameters "]" .
```

### 2.14 §4.9.3 取负表达式 — `NegateExpression` (L4702–4704)

```ebnf
NegateExpression = "-" Expression .
```

### 2.15 §4.9.4 逻辑非表达式 — `LogicalNotExpression` (L4748–4750)

```ebnf
LogicalNotExpression = "!" Expression .
```

### 2.16 §4.9.5 按位取反表达式 — `BitwiseNotExpression` (L4768–4770)

```ebnf
BitwiseNotExpression = "^" Expression .
```

### 2.17 §4.9.6 引用表达式 — `ReferenceExpression` (L4791–4793)

```ebnf
ReferenceExpression = "&" Expression .
```

### 2.18 §4.9.7 解引用表达式 — `DereferenceExpression` (L4846–4848)

```ebnf
DereferenceExpression = "*" Expression .
```

### 2.19 §4.9.8 乘法表达式 — `MultiplyExpression` (L4866–4868)

```ebnf
MultiplyExpression = Expression "*" Expression .
```

### 2.20 §4.9.9 除法表达式 — `DivideExpression` (L4888–4890)

```ebnf
DivideExpression = Expression "/" Expression .
```

### 2.21 §4.9.10 求余表达式 — `RemainderExpression` (L4910–4912)

```ebnf
RemainderExpression = Expression "%" Expression .
```

### 2.22 §4.9.11 加法表达式 — `AddExpression` (L4932–4934)

```ebnf
AddExpression = Expression "+" Expression .
```

### 2.23 §4.9.12 减法表达式 — `MinusExpression` (L4954–4956)

```ebnf
MinusExpression = Expression "-" Expression .
```

### 2.24 §4.9.13 逻辑左移表达式 — `LogicalLeftShiftExpression` (L4976–4978)

```ebnf
LogicalLeftShiftExpression = Expression "<<" Expression .
```

### 2.25 §4.9.14 逻辑右移表达式 — `LogicalRightShiftExpression` (L5002–5004)

```ebnf
LogicalRightShiftExpression = Expression ">>" Expression .
```

### 2.26 §4.9.15 小于表达式 — `LessExpression` (L5028–5030)

```ebnf
LessExpression = Expression "<" Expression .
```

### 2.27 §4.9.16 大于表达式 — `GreaterExpression` (L5050–5052)

```ebnf
GreaterExpression = Expression ">" Expression .
```

### 2.28 §4.9.17 小于等于表达式 — `LessEqualExpression` (L5072–5074)

```ebnf
LessEqualExpression = Expression "<=" Expression .
```

### 2.29 §4.9.18 大于等于表达式 — `GreaterEqualExpression` (L5097–5099)

```ebnf
GreaterEqualExpression = Expression ">=" Expression .
```

### 2.30 §4.9.19 相等表达式 — `EqualExpression` (L5122–5124)

```ebnf
EqualExpression = Expression "==" Expression .
```

### 2.31 §4.9.20 不等表达式 — `NotEqualExpression` (L5144–5146)

```ebnf
NotEqualExpression = Expression "!=" Expression .
```

### 2.32 §4.9.21 按位与表达式 — `BitwiseAndExpression` (L5167–5169)

```ebnf
BitwiseAndExpression = Expression "&" Expression .
```

### 2.33 §4.9.22 按位异或表达式 — `BitwiseXorExpression` (L5191–5193)

```ebnf
BitwiseXorExpression = Expression "^" Expression .
```

### 2.34 §4.9.23 按位或表达式 — `BitwiseOrExpression` (L5215–5217)

```ebnf
BitwiseOrExpression = Expression "|" Expression .
```

### 2.35 §4.9.24 逻辑和表达式 — `LogicalAndExpression` (L5239–5241)

```ebnf
LogicalAndExpression = Expression "&&" Expression .
```

### 2.36 §4.9.25 逻辑或表达式 — `LogicalOrExpression` (L5259–5261)

```ebnf
LogicalOrExpression = Expression "||" Expression .
```

### 2.37 §4.9.26 空值合并表达式 — `NullCoalescingExpression`, `CoalescingAlternative` (L5279–5285)

```ebnf
NullCoalescingExpression = Expression "??" CoalescingAlternative .

CoalescingAlternative = Expression
                      | "throw" [ Expression ]
                      | "return" [ Expression ] .
```

### 2.38 §4.9.27 条件表达式 — `ConditionalExpression` (L5385–5387)

```ebnf
ConditionalExpression = Expression "?" Expression ":" Expression .
```

---

## 3. Operator table (single consolidated view)

Precedence levels are exactly the 16 rows of L4513–4528; **higher number = lower precedence**. All rows are **left-associative at the same level** (L4535) except `??` and `?:`, which are **right-associative** (L4536). Postfix binds before unary; unary binds before multiplicative and, together with its operand, before any binary (L4541–4543). Homographic `& * ^ -` are unary **in operand position** and binary **after an operand** (L4547–4552).

| Token(s) | Prec | Assoc | Operand types (spec) | Result type | Evaluation order | Short-circuit | Overload | Special cases (lines) |
|---|---|---|---|---|---|---|---|---|
| `( )` grouping | 1 | n/a | any `Expression` | inner expression's type | inner first | no | n/a | returns inner value; only way to override precedence (L3666, L3674–3676, L4537) |
| `.` member access | 2 postfix | left | source: identifier / type / instance / module | member type (field/const value, or accessor read result) | source, then member read | no | not an operator overload; member resolution | member must be on source type/inheritance chain; static only for type sources; accessor issue OQ-1 (L3792–3817) |
| `->` pointer member access | 2 postfix | left | source instance type MUST be pointer type | pointee's member type | source, deref, member access | no | none | forbidden if pointee type omitted; assignment overwrites target member storage (L3838–3851) |
| `[ ]` index access | 2 postfix | left | source: array type **or** type with index accessor; index: cardinal or implicitly convertible to cardinal | element type (array) / indexer target type | source, index, then get | no | **index accessor** (`operator [...]`) is one of the 12 | bounds check; `OutOfRangeException`; `set` for assignment (L3867–3896) |
| `<...>` monomorphization | 2 postfix | left | defined §10.2 (outside range) | §10.2 | §10.2 | no | n/a | appears in `ConstructedTypeName` (L4019) |
| `( )` call | 3 | left | callee return type MUST be function type | function return type | callee, then args | no | overload resolution for identifier callee (§8.5) | callee identifier MUST name accessible function or compile-time function (L3910–3927) |
| construction `T(...)` | 3 | left | `ConstructedTypeName` = `Type` \| `MonomorphizationExpression`; class/structure only | constructed instance | args, then ctor with destination as `this` | no | ctor overload (§8.5) | no allocation specifier; in-place on RHS of def/assignment (L4032–4055) |
| member construction `T.Name(expr)` | 3 | left | variant/union; `Name` value-carrying for variant; expr → member type | constructed instance | expr, then tag/payload write | no | none | MUST NOT parse as call; non-value member → member access (L4127–4152) |
| unary `-` negate | 4 | prefix | operand type MUST have negate overload | negate overload's return type | operand | no | **yes** (`-` unary) | compile-time dispatch; no implicit conversion rule stated (L4713–4720) |
| `!` logical not | 4 | prefix | `boolean` or implicitly convertible to `boolean` | `boolean` | operand | no | **no** (not in overload set) | `true`→`false`, else `true` (L4755–4762) |
| unary `^` bitwise not | 4 | prefix | integer / cardinal / pointer | same as operand | operand | no | **no** | pointer case applies to pointer itself (L4779–4785) |
| `&` reference | 4 | prefix | operand MUST be addressable | pointer to operand type | address computation | no | **no** | addressable set: identifier, `*ptr`, field member access on addressable base, builtin index on addressable array (L4798–4810) |
| unary `*` dereference | 4 | prefix | dereferenceable pointer, or implicitly convertible to one | pointee instance | operand (address) then load | no | **no** | yields the instance at the address (L4853–4860) |
| `*` multiply | 5 | left | MUST exist matching `*` overload for both operand types | overload return type | left, right, call | no | **yes** | no implicit-conversion rule stated here (L4877–4882) |
| `/` divide | 5 | left | matching `/` overload | overload return | left, right, call | no | **yes** | builtin behavior per §3.2 (L4899–4904) |
| `%` remainder | 5 | left | matching `%` overload | overload return | left, right, call | no | **yes** | builtin behavior per §3.2 (L4921–4926) |
| `+` add | 6 | left | matching `+` overload | overload return | left, right, call | no | **yes** | unary `+` does not exist in `Expression` (L3544–3579) |
| binary `-` subtract | 6 | left | matching `-` overload | overload return | left, right, call | no | **yes** | same symbol as unary negate, distinguished by arity (L4636) |
| `<<` logical left shift | 7 | left | both: integer/cardinal/pointer; right additionally cardinal or implicitly convertible to cardinal | **first operand's type** | left, right | no | **no** | pointer: operate on pointer itself (L4983–4996) |
| `>>` logical right shift | 7 | left | same as `<<` | **first operand's type** | left, right | no | **no** | "logical" right shift (L5009–5022) |
| `<` less | 8 | left | matching `<` overload | overload return type (derived requirement: boolean, OQ-11) | left, right, call | no | **yes** | required by `<=` too (L5039–5044) |
| `>` greater | 8 | left | matching `>` overload | overload return (boolean, OQ-11) | left, right, call | no | **yes** | required by `>=` too (L5061–5066) |
| `<=` less-equal | 8 | left | requires both `<` and `==` overloads | `boolean` (result of desugared `\|\|`) | operands once, `<` then (if false) `==` | **yes** — `==` only when `<` is false (L5090 + L5272) | **derived, cannot be defined** | MUST NOT be defined as negation of `>`; NaN → `false` (L4627–4631) |
| `>=` greater-equal | 8 | left | requires both `>` and `==` overloads | `boolean` | `>` then (if false) `==` | **yes** | **derived, cannot be defined** | NaN → `false` (L4628–4631, L5115) |
| `is` | 8 | left | left: any expression; target: class-type identifier / interface / variant member / `illegal` / `[ - ] "infinite"` | `boolean` | left first, then test | no | **no** (MUST NOT be overloaded) | statically impossible → `false`; special targets require float left (L4251–4267) |
| `==` equal | 9 | left | matching `==` overload | overload return (boolean for float builtin) | left, right, call | no | **yes** | `!=` derives from it (L5133–5138) |
| `!=` not-equal | 9 | left | matching `==` overload | `boolean` | left, right, `==`, then `!` | no | **derived, cannot be defined** | `!(==)`; IEEE 754 NaN semantics (L4626, L4629, L5160) |
| `&` bitwise and | 10 | left | both integer/cardinal/pointer; same type or one implicitly convertible to the other | common operand type (OQ-4) | left, right, op | no | **no** | pointer: operate on pointer itself (L5174–5185) |
| `^` bitwise xor | 11 | left | same as `&` | common operand type (OQ-4) | left, right, op | no | **no** | conflicts with unary `^` only by position (L5198–5209) |
| `\|` bitwise or | 12 | left | same as `&` | common operand type (OQ-4) | left, right, op | no | **no** | L5222–5233 |
| `&&` logical and | 13 | left | both `boolean` or implicitly convertible to `boolean` | `boolean` | left; right **only if** left true | **yes** (MUST NOT evaluate right when left false) | **no** | L5246–5253 |
| `\|\|` logical or | 14 | left | both `boolean` or implicitly convertible to `boolean` | `boolean` | left; right **only if** left false | **yes** (MUST NOT evaluate right when left true) | **no** | L5266–5273 |
| `??` null-coalescing | 15 | **right** | left type MUST define `operator ??` returning `Result<TValue; TAlternative>`; right = `CoalescingAlternative` | `TValue` | left **exactly once**; right lazily only on `Alternative` | **yes** (lazy continuation) | **special** (`operator ??`, not a binary operator overload) | `throw` / `return` forms; empty-type rules; right side not an `operator ??` parameter (L5296–5327) |
| `?:` conditional | 16 | **right** | condition `boolean`/convertible; branches same type or one convertible to the other | common type of branches | condition, then exactly one branch | **yes** (branches MUST NOT both be evaluated) | **no** | derived/never overloaded (L5398–5412) |

**Unary/postfix binding summary (L4539–4552):**
- postfix (`.`, `->`, `[]`, monomorphization) binds **before** unary (L4541);
- unary binds **before** multiplicative `* / %` (L4542);
- unary + its operand bind **before any binary operator** (L4543);
- `&`, `*`, `^`, `-` are unary iff in operand position, binary iff after an operand (L4547–4552).
- Consequence: `-a * b` = `(-a) * b`; `-a + b` = `(-a) + b`; `*p.f` = `*(p.f)` (postfix wins); `&a[0]` = `&(a[0])`; `-x is C` = `(-x) is C` (unary > level-8 `is`).

---

## 4. Semantic rules (precise bullets, tagged)

### 4.0 Whole-expression evaluation order — L3623–3628

- **S-ORD-1** Subexpressions MUST be evaluated before the expression that contains them (L3625).
- **S-ORD-2** Sibling subexpressions (mutually non-nested subexpressions of the same expression) MUST be evaluated left-to-right in written order (L3626–3627).
- **S-ORD-3** `&&`, `||`, `??`, `?:` MUST follow their own evaluation rules, which override naive sibling order by making the RHS conditional (L3628, L5252, L5272, L5302–5304, L5408).
- **S-ORD-4** Operator binding order is per §4.9.1 (L3619–3621).

### 4.1 Value expressions — L3630–3659

- **S-VAL-1** An `Identifier` MUST already be defined and accessible (L3646–3647).
- **S-VAL-2** `Literal` yields the value it denotes (L3657); literal forms are §2.7.
- **S-VAL-3** `MemberAccessExpression` / `PointerMemberAccessExpression` / `MonomorphizationExpression` yield the access expression's value (L3658, §4.4 / §10.2).
- **S-VAL-4** `IndexAccessExpression` yields the content it accesses (L3659, §4.4.3).
- **S-VAL-5** Value expressions are precedence level 1 (highest) together with parenthesized, compile-time, and cast expressions (L4513).

### 4.2 Parenthesized expressions — L3661–3676

- **S-PAR-1** `( Expression )` yields the inner expression's value (L3676); it changes evaluation precedence only (L4537).
- **S-PAR-2** No temporariness/lvalue change is stated; see OQ-2.1 (addressability example uses `&pair.X` and `&numbers[0]`, and rejects `&(count + 1)` at L4834).

### 4.3 Type conversion expressions — L3678–3750

- **S-CVT-1** Syntax is exactly `Cast` `<` `Type` `>` `(` `Expression` `)` (L3683). `Cast` is the guide token (L3688); `Type` is the target type (L3689); `Expression` is the converted value (L3690).
- **S-CVT-2** The expression's result type MUST be `Type` (L3694).
- **S-CVT-3** Resolution order: (1) choose a **conversion operator** that accepts the converted value (L3698); (2) if none exists, follow the explicit-conversion rules of the involved type pages (L3699); (3) an explicit conversion MUST exist between the value's type and `Type`, otherwise the compiler MUST error (L3700–3701).
- **S-CVT-4** Explicit conversion MUST happen **only** through a type-conversion expression (L3705).
- **S-CVT-5** Conversion operators carrying `implicitable` MAY be invoked implicitly at type-mismatch sites (L3706, §9.7; also L4424).
- **S-CVT-6** The type-conversion expression itself MUST NOT be overloaded (L3707); conversion operators are distinct declarations (L4420–4424).
- **S-CVT-7** Conversion-operator declaration rules: return type MUST equal the conversion target type; its parameter MUST accept the converted value (L4401–4402); it MUST be invoked through a cast expression (L4423).
- **S-CVT-8** Examples: int narrowing, bool→numeric, user HSLColor→RGBColor operator preferred (L3711–3750).

### 4.4 Access expressions — L3752–3896

**4.4.1 Member access (L3773–3818)**
- **S-MEM-1** Source MUST be an identifier, type, instance, or module (L3792).
- **S-MEM-2** "Source's type" = the type of the instance carried by the source (L3793); for a type source it is the type itself (L3795); for a module it does not apply (L3794).
- **S-MEM-3** Non-module source: the member MUST be on the source's type or its inheritance chain (L3796). Module source: the member MUST be a declaration in the module (L3797).
- **S-MEM-4** Type source: member MUST be `static`, an enumerator, or a payload-less variant state (L3798). Non-type source: member MUST NOT be `static` (L3799).
- **S-MEM-5** Member resolution order is: (1) members of the source's type; (2) base-class members; (3) members of implemented interfaces (L3805–3808).
- **S-MEM-6** Methods MUST be called through a member-access expression (L3809); function fields likewise (L3810).
- **S-MEM-7** Accessor members MUST NOT be accessed through a member-access expression (L3811) — directly contradicted by **S-MEM-8**; see OQ-1.
- **S-MEM-8** Return: field → its value; constant → its value; accessor → read the accessor and yield the read result (L3815–3817).
- **S-MEM-9** `Name` MUST point to a member in the source's type (L3788).

**4.4.2 Pointer member access (L3819–3852)**
- **S-PTR-1** The type of the instance carried by the source MUST be a pointer type (L3838); i.e. deref-then-access is mandatory (L3845).
- **S-PTR-2** Member MUST be on the pointee type or its inheritance chain (L3839); MUST NOT be `static` (L3840).
- **S-PTR-3** Forbidden when the pointee type is omitted/unknown (L3846).
- **S-PTR-4** As an assignment target it MUST overwrite the target member's storage (L3851).
- **S-PTR-5** Otherwise follows 4.4.1 (L3847).

**4.4.3 Index access (L3853–3897)**
- **S-IDX-1** Index expression type MUST be a cardinal type or implicitly convertible to cardinal (L3867).
- **S-IDX-2** Source's instance type MUST be an array type (L3871) or carry an index accessor (L3871–3872).
- **S-IDX-3** Array → built-in index access, yielding the element (L3876); result type = element type (L3881).
- **S-IDX-4** Otherwise the index value MUST be substituted into the index accessor and executed (L3877); result type = the accessor's target type (L3882).
- **S-IDX-5** Assignment: array → overwrite the element's memory (L3886); indexer → call its `set` (L3887).
- **S-IDX-6** Out-of-bounds is defined as index value not less than array size (L3891). Built-in access MUST check; on out-of-bounds it MUST raise a runtime `OutOfRangeException` (L3892–3893).
- **S-IDX-7** Compiler MUST warn when it proves the index always out of bounds (L3894).
- **S-IDX-8** Compiler MUST remove the check when it proves the index in bounds, **except** when the index is inside a `verbatim` block (L3895–3896).

### 4.5 Function call expressions — L3898–4010

- **S-CALL-1** Callee expression's result type MUST be a function type (L3910).
- **S-CALL-2** Identifier callee MUST point to an accessible function or compile-time function (L3911).
- **S-CALL-3** Member-access callee MUST call the function through that member access (L3912; methods §8.4, function fields §3.8.1).
- **S-CALL-4** If the identifier callee points to several same-name function constants, the callee MUST be selected by overload resolution (§8.5, L3918).
- **S-CALL-5** Arguments MAY be omitted iff a function identifier is used (L3922) — text conflicts with examples L3947/L3993; see OQ-3.
- **S-CALL-6** Result of the call expression is the function's return value (L3927).
- **S-CALL-7** Legal argument forms demonstrated: positional (L3953); omitted defaulted args (L3956); named (L3959); named out of order (L3962); mixed positional-then-named (L3965); reference parameter mutated through call (L3968–3969); immediately-invoked function literal (L3971–3976).
- **S-CALL-8** Illegal argument forms demonstrated: positional after named (L3981–3983); duplicate argument (L3995–3997); missing required argument (L3999–4001); calling a non-function (L4003–4006).
- **S-CALL-9** A function pointer call `(*functionPrinter)()` is legal (L3991–3993); a zero-arg call through a function-typed parameter `callback()` is legal (L3947).
- **S-CALL-10** Call/construction/member-construction share precedence level 3 (L4515).

### 4.6 Construction expressions — L4012–4108

- **S-NEW-1** `ConstructedTypeName` = `Type` or `MonomorphizationExpression` (L4019, L4025–4027).
- **S-NEW-2** The constructed type MUST be a class (L4032) or structure (L4032); MUST NOT be an interface (L4033), an enumeration (L4034), a primitive (L4036), or a type constructor (L4037).
- **S-NEW-3** Variants and unions MUST be constructed via member-construction expressions (L4035).
- **S-NEW-4** The type MUST declare a constructor matching `Arguments` (L4038). Arguments MAY be omitted iff an empty-parameter constructor exists (L4039).
- **S-NEW-5** A construction expression MUST NOT specify an allocation method (L4043; e.g. `heap Point(...)` illegal, L4105–4107).
- **S-NEW-6** On the RHS of a definition or assignment, the instance MUST be constructed directly into the target storage of that definition/assignment; constructing an intermediate then overwriting the target MUST NOT happen (L4044–4045).
- **S-NEW-7** Otherwise the compiler MUST allocate storage for the instance (L4046, §12.1).
- **S-NEW-8** Behavior: call the constructor with the target storage as `this` and the arguments as the remaining parameters (L4050); member initialization per the constructed type's lifetime MUST complete before the constructor call (L4051).
- **S-NEW-9** Result: the fully constructed instance (L4055).
- **S-NEW-10** Demonstrated: struct construction, reference-type default construction, overwrite-on-assign (L4085–4094); illegal interface construction, arity mismatch (L4097–4103).

### 4.7 Member construction expressions — L4110–4223

- **S-MCT-1** The constructed type MUST be a variant (L4127) or union (L4127); `Name` MUST be a member (L4128).
- **S-MCT-2** For a variant, `Name` MUST be a payload-carrying state (L4129). `Expression`'s type MUST equal the member's type or be implicitly convertible to it (L4130).
- **S-MCT-3** Parse rule: member access whose source is a type and whose target is a value-carrying member → member-construction expression (L4134); value-less member → member-access expression (L4135); MUST NOT be parsed as a call (L4136).
- **S-MCT-4** No allocation method may be specified (L4140). RHS-of-definition/assignment MUST construct in place, no intermediate (L4141–4142); otherwise compiler-allocated storage (L4143).
- **S-MCT-5** Variant: the named member MUST become the instance's current state, and the instance's payload MUST be the value of `Expression` (L4147).
- **S-MCT-6** Union: the named member MUST be written starting at the instance's memory start, and the remaining members MUST NOT be initialized (L4148).
- **S-MCT-7** Result: the constructed instance (L4152).
- **S-MCT-8** Demonstrated: union member + heap allocation, variant payload state, payload-less state as plain member value (L4196–4207); illegal `Status.Done()` (payload-less), wrong payload type, unknown member (L4212–4222).

### 4.8 `is` expressions — L4225–4333

- **S-IS-1** `IsTarget` is an `Identifier` (class type, interface, or variant member) or a `SpecialFloatTarget` = `IllegalLiteral` | `InfinityTarget` (L4230–4236).
- **S-IS-2** The identifier MUST point to a class type, an interface, or a variant member (L4251). Result type MUST be `boolean` (L4252).
- **S-IS-3** With a `SpecialFloatTarget`, the left operand's type MUST be a float type (L4253).
- **S-IS-4** Class target: true iff the left operand's **runtime** type is that class or derives from it (L4257).
- **S-IS-5** Interface target: true iff the runtime type implements that interface (L4258).
- **S-IS-6** Variant-member target: true iff the left operand's **current state** is that member (L4259).
- **S-IS-7** `illegal` target: true iff the value is an IEEE 754 NaN (L4260).
- **S-IS-8** `infinite` / `-infinite` target: true iff the value is an IEEE 754 infinity with matching sign (L4261).
- **S-IS-9** If the left operand's static type makes the test impossible, the result MUST be `false` (L4262).
- **S-IS-10** `is` MUST NOT be overloaded and MUST NOT be affected by operator overloading (L4266–4267).
- **S-IS-11** Demonstrated: class test, interface test, variant state, NaN, +inf/−inf, statically-impossible `1 is Cat` → `false` (L4298–4321); illegal non-type target `1 is integer(4)`, illegal non-float NaN test `1 is illegal` (L4326–4332).

### 4.9 Operator definitions — L4335–4505

- **S-ODEF-1** Modifier list MUST precede `operator` (L4371); the operator notation MUST be adjacent to `operator`, with no newline or comment between them (L4372); the symbol MUST be written as itself, unquoted (L4373).
- **S-ODEF-2** `OperatorShorthand` is allowed only for `OperatorSymbol` (L4374). It MUST expand to `OperatorNotation ":" "function(" Parameters ")" Type` (L4375). Shorthand with omitted `:` has return type **empty type**, which MUST NOT be inferred (L4376).
- **S-ODEF-3** At least one of `:` and `=` MUST appear (L4377).
- **S-ODEF-4** Except contract operators in interface declaration bodies, operator definitions MUST be declared inside an extension block (L4378).
- **S-ODEF-5** Base accessibility of an operator is `public` (L4379); operator definitions MUST NOT carry accessibility modifiers (L4380).
- **S-ODEF-6** Inside an extension block, parameters MAY access private members of the extension's target type (L4381).
- **S-ODEF-7** The same operator MUST NOT be defined twice in one type; two definitions with the **same notation and parameter list** are the same operator (L4382–4383).
- **S-ODEF-8** With `:` the operator type MUST be `Type` (L4387); without shorthand and with `:` omitted, the type MUST be inferred from `=`'s return value (L4388); the operator type MUST be a function type (L4389).
- **S-ODEF-9** Non-indexer notation: 1 or 2 parameters (L4390). 2 → binary; first accepts the left operand, second the right (L4391–4393). 1 → unary; its parameter accepts the operand (L4394–4395).
- **S-ODEF-10** Unary user operators MUST be only `-`, `??`, or a conversion operator (L4396).
- **S-ODEF-11** The operator type MUST NOT contain a `this` parameter (L4397).
- **S-ODEF-12** Return type MUST be the result type, except for `??` (L4398).
- **S-ODEF-13** Indexer notation: `:` MUST appear; parameter list = `Parameters`; return type = `Type` (L4399); any number of index parameters (L4400).
- **S-ODEF-14** Conversion-target notation: return type MUST equal the conversion target; the parameter MUST accept the converted value (L4401–4402).
- **S-ODEF-15** `??`: operator type MUST have exactly one parameter, returning `Result<TValue; TAlternative>` (L4406); the parameter accepts the left operand (L4407); the right-hand continuation MUST NOT be a parameter of `operator ??` (L4408); `operator ??` MUST NOT be invoked as an ordinary unary operator (L4409).
- **S-ODEF-16** Index accessor: MUST appear in a class/structure extension block; interface-declaration-body indexers are contracts and are declared directly (L4414); its body MUST be `AccessorMembers` (L4415); accessor members are methods and the `this` parameter MUST be written explicitly (L4416); MUST NOT be `static` (L4417); `get` and `set` MUST provide initialization expressions (L4418).
- **S-ODEF-17** Conversion operator: MUST be called via a type-conversion expression (L4423); with `implicitable` it MAY be called implicitly (L4424).
- **S-ODEF-18** Overloading: multiple definitions with the same symbol MAY coexist (L4428); one unary **and** one binary for the same symbol MAY coexist (L4429); parameter types among same-symbol definitions MUST be pairwise different (L4430).
- **S-ODEF-19** Demonstrated: binary `+`, indexer with get/set in an extension block, `==` (L4450–4479); illegal arity 3, `this` parameter, `&` symbol, accessibility modifier, global duplicate, indexer outside extension block (L4482–4504).

### 4.9.1 Precedence and binding — L4507–4586

- **S-PREC-1** 16 levels; higher level binds first between adjacent levels (L4534); same level is left-associative (L4535); `??` and `?:` are right-associative (L4536); parentheses may override (L4537).
- **S-PREC-2** Unary binding rules per the §3 table ("Unary/postfix binding summary") and L4541–4543.
- **S-PREC-3** Homographic disambiguation per position for `&`, `*`, `^`, `-` (L4547–4552).
- **S-PREC-4** Demonstrated: `-2 * 3 = -6`; `1 + 2 * 3 = 7`; `1 + 2 << 3` = `(1 + 2) << 3`; `1 is Cat == false` = `(1 is Cat) == false`; parens override (L4565–4578).

### 4.9.2 Overloading — L4588–4696

- **S-OVL-1** Exactly **twelve** overloadable entities (L4594–4605, L4616): binary `+`, binary `-`, `*`, `/`, `%`, `==`, `>`, `<`, unary `-`, indexer `[...]`, conversion operator (type name), `??`.
- **S-OVL-2** An overload is an operator definition for that symbol (L4610); it MUST be declared by an operator definition (L4611); it MUST be resolved at compile time (L4612).
- **S-OVL-3** Unary overloads MUST be only unary `-`, `??`, or a conversion operator (L4617).
- **S-OVL-4** Bitwise operators (§4.9.21–23), logical operators (§4.9.24), and shift operators (§4.9.13) MUST NOT be overloaded (L4618). `is` MUST NOT be overloaded (L4266). `?:` MUST NOT be overloaded (L5412). `Cast` (the cast expression) MUST NOT be overloaded (L3707).
- **S-OVL-5** The operator symbol MUST match the corresponding overload (L4619).
- **S-OVL-6** Derived operators: `!=` derives from `==`; `<=` and `>=` derive from `<` and `>` (L4623–4624). Derived operators MUST NOT be defined as operators (L4625).
- **S-OVL-7** Derived truth rules: `!=` = `!(==)` (L4626); `<=` = `(<) || (==)` (L4627); `>=` = `(>) || (==)` (L4628). All MUST follow IEEE 754, where NaN is unequal to everything including itself (L4629). `<=` MUST NOT be defined as the negation of `>`; with a NaN operand `>`'s negation is `true` but `<=` MUST be `false` (L4630–4631).
- **S-OVL-8** Dispatch MUST use overload resolution (§8.5) with the overloaded name being the operator symbol (L4635); unary vs binary is distinguished by the definition's parameter count (L4636); dispatch MUST complete at compile time (L4637).
- **S-OVL-9** `!=`, `>=`, `<=` MUST NOT require their own overloads (L4641); they MUST substitute the table's overloads into their own desugared form (L4642).
- **S-OVL-10** Built-in overloads: provided intrinsically by the compiler, MUST NOT be written as operator definitions (L4646–4648), MUST NOT be supplied by the Built-In Library (L4649); their set and behavior MUST be defined by the type pages (L4650–4651); dispatch MUST treat built-in and user-defined overloads identically (L4652).
- **S-OVL-11** Demonstrated: `+` overload match, definition outside extension block illegal, `a + 1` with no match illegal (L4671–4694).

### 4.9.3 Unary `-` (negate) — L4698–4742

- **S-NEG-1** The operand type MUST have a negate overload (L4713); the result type MUST be that overload's return type (L4714); dispatch MUST be at compile time (L4718–4719); built-in behavior per §3.2 (L4720).
- **S-NEG-2** Demonstrated: literal `-5`, variable `-value`, `-infinite`, illegal `-true` (L4728–4740).

### 4.9.4 Logical not `!` — L4744–4762

- **S-NOT-1** Operand MUST be `boolean` or implicitly convertible to `boolean` (L4755); result MUST be `boolean` (L4760); `true`→`false`, otherwise `true` (L4761–4762).

### 4.9.5 Bitwise not unary `^` — L4764–4785

- **S-BNOT-1** Operand MUST be an integer, cardinal, or pointer type (L4779); for pointers the operation MUST apply to the pointer itself (L4780); result type = operand's type (L4785).

### 4.9.6 Reference `&` — L4787–4840

- **S-REF-1** Operand's value MUST be addressable (L4798).
- **S-REF-2** Addressable iff one of: an identifier (L4800); a pointer dereference (L4801); an addressable member access whose member is a **field** (L4802); an addressable built-in index access (L4803). Values outside this list MUST NOT be addressed (L4804).
- **S-REF-3** Result is the memory address of the operand's value; result type is a pointer (L4809–4810).
- **S-REF-4** Demonstrated: `&count`, `&pair.X`, `&numbers[0]` legal; `&(count + 1)`, `&Point().X` illegal (L4822–4839).

### 4.9.7 Dereference unary `*` — L4842–4860

- **S-DER-1** The operand type MUST be a dereferenceable pointer (L4854) or implicitly convertible to one (L4855).
- **S-DER-2** Result is the instance located at the address held by the operand (L4860).

### 4.9.8–4.9.12 Multiplicative/additive `/ * % + -` — L4862–4970

- **S-ARITH-1** For each, a matching overload for the left and right expression types MUST exist (L4877, L4899, L4921, L4943, L4965).
- **S-ARITH-2** The left expression is the first argument and the right expression the second; the overload is called (L4878, L4900, L4922, L4944, L4966).
- **S-ARITH-3** The expression's result is the overload's return value (L4882, L4904, L4926, L4948, L4970).
- **S-ARITH-4** No implicit-conversion rule is stated for these operands; overload resolution (§8.5) governs applicability (OQ-6). Built-in behavior is per §3.2.

### 4.9.13–4.9.14 Shifts `<<`, `>>` — L4972–5022

- **S-SH-1** The shift amount (second operand) MUST be a cardinal type or implicitly convertible to cardinal (L4983–4985, L5009–5011).
- **S-SH-2** Both operands MUST be integer, cardinal, or pointer types (L4990, L5016); pointer operands operate on the pointer itself (L4991, L5017).
- **S-SH-3** The result is the left operand shifted by the right operand's value (L4995, L5021); result type is the **first** operand's type (L4996, L5022).
- **S-SH-4** Not overloadable (L4618).

### 4.9.15–4.9.20 Comparisons and equality `<`, `>`, `<=`, `>=`, `==`, `!=` — L5024–5161

- **S-CMP-1** `<` and `>` require a matching overload and yield that overload's return value (L5039–5044, L5061–5066).
- **S-CMP-2** `<=` requires both `<` and `==` overloads (L5083–5084); `>=` requires both `>` and `==` overloads (L5108–5109).
- **S-CMP-3** `<=` calls `<` then `==` (both with left as first, right as second argument) (L5085–5086); substitutes the two results as the operands of a logical-or (L5090); the logical-or result is the `<=` result (L5091). Same for `>=` with `>` (L5110–5116).
- **S-CMP-4** `==` requires a matching overload and yields its return value (L5133–5138).
- **S-CMP-5** `!=` requires a matching `==` overload (L5155); the equality result is fed into logical-not (L5160); the logical-not result is the `!=` result (L5161).
- **S-CMP-6** Because the desugaring targets `&&`/`||`/`!` (which require `boolean` operands), the `<`, `>`, `==` overloads used by `<=`, `>=`, `!=` must produce `boolean`; see OQ-11.
- **S-CMP-7** `<=` is **not** `!(>)` (L4630–4631).
- **S-CMP-8** None of `<=`, `>=`, `!=` may be defined by the user (L4625), and none of them require their own overload (L4641).

### 4.9.21–4.9.23 Bitwise `&`, `^`, `|` — L5163–5233

- **S-BIT-1** The two operand types MUST be identical, or one MUST be implicitly convertible to the other (L5174, L5198, L5222).
- **S-BIT-2** Both operands MUST be integer, cardinal, or pointer types (L5179, L5203, L5227); pointer operands operate on the pointer itself (L5180, L5204, L5228).
- **S-BIT-3** Result is the respective bitwise operation applied to the operand values (L5184, L5208, L5232); result type is "the type of `Expression`" (L5185, L5209, L5233) — singular for a binary operation; OQ-4.
- **S-BIT-4** Not overloadable (L4618).
- **S-BIT-5** `&`, `^` also appear as unary operators; disambiguation is by position (L4547–4552).

### 4.9.24–4.9.25 Logical `&&`, `||` — L5235–5273

- **S-LOG-1** Both operands MUST be `boolean` or implicitly convertible to `boolean` (L5246, L5266); result MUST be `boolean` (L5251, L5271).
- **S-LOG-2** `&&`: if the first operand converts to `false`, the result MUST be `false` and the second operand MUST NOT be evaluated (L5252); otherwise the second operand MUST be converted to `boolean` and returned (L5253).
- **S-LOG-3** `||`: if the first operand converts to `true`, the result MUST be `true` and the second operand MUST NOT be evaluated (L5272); otherwise the second operand MUST be converted to `boolean` and returned (L5273).
- **S-LOG-4** Not overloadable (L4618).

### 4.9.26 Null-coalescing `??` — L5275–5379

- **S-NC-1** Left operand type MUST define `operator ??` (L5296); that operator MUST be a unary decision operation whose return type MUST be `Result<TValue; TAlternative>` (L5297); the left operand MUST be evaluated **exactly once** (L5298).
- **S-NC-2** `Result.Value(v)` → the whole expression MUST take `v` and MUST NOT evaluate `CoalescingAlternative` (L5302).
- **S-NC-3** `Result.Alternative(a)` → execute per the alternative form (L5303):
  - **S-NC-3a** `Expression` → MUST be evaluated lazily, and its type MUST be `TValue` (L5304);
  - **S-NC-3b** `throw` → MUST throw `a`; if `TAlternative` is an empty type, this form MUST error (L5305);
  - **S-NC-3c** `throw Expression` → MUST throw that expression; `a` MUST NOT be used (L5306);
  - **S-NC-3d** `return` → MUST return the entire decision `Result` from the current function; the current function's return type MUST be that `Result`'s type (L5307);
  - **S-NC-3e** `return Expression` → MUST return that expression from the current function; its type MUST equal the current function's return type (L5308).
- **S-NC-4** Result type MUST be `TValue` (L5309). `throw` forms obey §5.10.3 positional limits; `return` forms obey §5.10.2 (L5310–5311).
- **S-NC-5** Built-ins: `Result<TValue; TAlternative>`'s `operator ??` is the identity decision (L5315); `Optional<T>`'s maps `Value(v)`→`Result<T;>.Value(v)` and `Null`→payload-less `Result<T;>.Alternative` (L5316); user types MAY define their own (L5317).
- **S-NC-6** Empty types: `Result<; T>`'s `Value` is an empty type; `Result<T;>`'s `Alternative` is an empty type (L5321). When `TAlternative` is empty, `?? throw` MUST NOT appear, while `?? throw Expression` is still allowed (L5322).
- **S-NC-7** `??` MUST NOT be overloaded as an ordinary binary operator (L5326); `operator ??` is a unary decision operation on the left operand's type; `CoalescingAlternative` MUST NOT participate in its selection (L5327).
- **S-NC-8** `??` is right-associative and precedence 15 (L4527, L4536).
- **S-NC-9** Demonstrated: `Optional` fallback `maybe ?? 0`; bare `?? throw` illegal for payload-less `Null`; `?? throw MyException()` inside `trap`; `Result` cases `?? throw`, `?? 0`, `?? return` (L5332–5378).

### 4.9.27 Conditional `?:` — L5381–5442

- **S-COND-1** The condition MUST be `boolean` or implicitly convertible to `boolean` (L5398).
- **S-COND-2** The two branch types MUST be identical, or one MUST be implicitly convertible to the other (L5399).
- **S-COND-3** The expression's type is the branches' common type: identical → that type; one derived from the other → the base one (L5400–5402).
- **S-COND-4** Condition `true` → the result MUST be the true-branch value (L5406); `false` → the false-branch value (L5407); both branches MUST NOT be evaluated (L5408).
- **S-COND-5** MUST NOT be overloaded (L5412). Right-associative, precedence 16 (L4528, L4536).
- **S-COND-6** Demonstrated: `Max`, sign extraction, illegal non-boolean condition, illegal branches with no common type (L5417–5441).

### 4.10 Implicit-conversion sites stated in this range

- **S-IMP-1** Index expression → cardinal (L3867).
- **S-IMP-2** Dereference operand → dereferenceable pointer (L4855).
- **S-IMP-3** `!` operand, `&&`/`||` operands, `?:` condition → `boolean` (L4755, L5246, L5266, L5398).
- **S-IMP-4** Member-construction value → member type (L4130).
- **S-IMP-5** `?:` branch → common type (L5399–5402).
- **S-IMP-6** Bitwise operand → other operand's type (L5174, L5198, L5222).
- **S-IMP-7** `implicitable` conversion operators at type-mismatch sites (L3706, L4424).
- **S-IMP-8** Overload resolution (§8.5, out of range) may additionally consider implicit conversions; the range does not enumerate them (OQ-6).

### 4.11 Lvalue / rvalue (addressability & assignment-target) semantics stated in this range

- **S-LV-1** Addressable: identifier, `*ptr`, field member access on an addressable base, built-in index access on an addressable source (L4799–4803). Nothing else is addressable (L4804).
- **S-LV-2** A pointer-member-access expression used as an assignment target MUST overwrite the target member's storage (L3851).
- **S-LV-3** An index-access expression used as an assignment target MUST either overwrite the array element's memory (built-in) or call the indexer's `set` (L3886–3887).
- **S-LV-4** `*ptr` denotes the instance at the address (L4860), i.e. it is usable as an lvalue; `&` of a temp is illegal (L4833–4839).
- **S-LV-5** A construction expression as the RHS of a definition/assignment must target that storage directly; no temporary-then-copy (L4044–4045, L4141–4142).
- **S-LV-6** Assignment statements themselves are §5.7 (out of range).

### 4.12 Illegal cases explicitly stated in this range

- `(x as T)` where no explicit conversion exists → compiler error (L3700–3701).
- Member access: member not in source type/inheritance chain; module member not a module declaration; static member via instance; non-static via type; accessor via member access (L3811) → errors (L3796–3811).
- Pointer member access on a non-pointer, untyped pointee, static member → error (L3838–3846).
- Indexing a non-array type without an index accessor; non-cardinal index without conversion → error (L3867–3872). Out-of-bounds runtime → `OutOfRangeException` (L3893).
- Call with non-function callee, inaccessible function identifier, empty-arg violation (OQ-3), named-after-positional, duplicate arg, missing required arg → error (L3910–3911, L3981–4006).
- Constructing interface/enum/primitive/type-constructor; variant/union via `T(...)`; missing matching ctor; allocation specifier → error (L4032–4043, L4097–4107).
- Member construction on a non-variant/non-union; non-member `Name`; payload-less variant state; non-convertible value; allocation specifier → error (L4127–4143, L4212–4222).
- `is` target not class/interface/variant member; `illegal`/`infinite` on non-float → error (L4251–4253, L4326–4332).
- Operator definition outside an extension block; accessibility modifier; `this` parameter; arity ≠ 1/2; unoverloadable symbol; duplicate definition; indexer outside extension block → error (L4378–4397, L4482–4504).
- `-` on a type without negate overload → error (L4713, L4738–4740).
- `&` on a non-addressable value → error (L4798–4804).
- `-infinite` target with a non-float left operand → error; `is` on non-float with `illegal` → error (L4253).
- `??` on a type without `operator ??`; `?? throw` when `TAlternative` is empty; `?? return` when the function's return type is not the `Result` type; `??` alternative type ≠ `TValue` → error (L5296–5311, L5322).
- `<=`/`>=`/`!=` defined as user operators → error (L4625).
- `?:` with non-boolean condition / branches with no common type → error (L5398–5399, L5432–5438).

---

## 5. Exact spellings introduced/used in this range

**Keywords / named tokens (case-sensitive as written):**
`Cast` · `is` · `infinite` (target word) · `illegal` (illegal-value literal) · `operator` · `implicitable` · `static` · `this` · `verbatim` · `throw` · `return` · `function` · `fixed` · `local` · `heap` · `dispose` · `Result` · `Optional` · `OutOfRangeException` · `Value` · `Alternative` · `Null` · `Construct` (constructor name in examples) · `get`/`set` (accessor members).

**Operator / punctuation spellings:** `.` · `->` · `[` `]` · `(` `)` · `<` `>` (type-argument brackets, e.g. `Cast<integer(2)>`) · `;` (generic argument separator in `Result<TValue; TAlternative>`, and omitted-argument form `Result<; T>` / `Result<T;>`) · `-` (unary negate and binary subtract) · `!` · `^` (unary not and binary xor) · `&` (reference and binary and) · `*` (deref and multiply) · `/` · `%` · `+` · `<<` · `>>` · `<` · `>` · `<=` · `>=` · `==` · `!=` · `|` · `&&` · `||` · `??` · `?` `:` · `=` (initializer / assignment; not an expression form).

**Operator symbols valid in `OperatorSymbol` (L4348):** `"+"`, `"-"`, `"*"`, `"/"`, `"%"`, `"=="`, `">"`, `"<"`, `"??"` — note: **no** `!`, `^` binary, `&`, `|`, `<<`, `>>`, `<=`, `>=`, `!=` in the definition symbol set; indexer and conversion operators use different notation forms.

---

## 6. Index of every `novasharp` example in the range

| # | Lines | Section | One-line description |
|---|---|---|---|
| 1 | 3711–3750 | 4.3 | `(a as integer(2))`, bool→int cast, and user conversion operator `RGBColor(from: HSLColor)` preferred over builtin rules |
| 2 | 3931–4010 | 4.5 | Calls: positional, default omission, named, reordered, mixed, reference parameter mutation, immediately-invoked function literal, zero-arg calls, pointer-to-function call, plus four commented illegal cases |
| 3 | 4059–4108 | 4.6 | `Point(0,0)`, `Cat()` default ctor, assignment overwrite; illegal interface construction, arity mismatch, `heap Point(...)` |
| 4 | 4156–4223 | 4.7 | Union `Pixel.RGBA(Color(...))`, heap+union, variant `Status.Loading(50)`, payload-less `Status.Done` as member value; illegal `Status.Done()`, wrong payload type, unknown member |
| 5 | 4271–4333 | 4.8 | `animal is Cat`, `is IComparable`, `status is Status.Loading`, `ratio is illegal`, `±infinite` sign tests, `1 is Cat` → false; illegal `1 is integer(4)`, `1 is illegal` |
| 6 | 4435–4505 | 4.9 | `operator +` on `Vector2`, indexer `operator [index: cardinal(4)]` with get/set, `operator ==`; illegal arity/`this`/`&`/`public`/global duplicate/indexer outside extension |
| 7 | 4556–4582 | 4.9.1 | Precedence demos: `-2*3`, `1+2*3`, `1+2<<3`, `1 is Cat == false`, parenthesized override |
| 8 | 4656–4696 | 4.9.2 | `a + b` resolves the `+` overload; illegal definition outside extension block; `a + 1` has no matching overload |
| 9 | 4724–4742 | 4.9.3 | Negate of literal, variable, `-infinite`; illegal `-true` |
| 10 | 4814–4840 | 4.9.6 | `&count`, `&pair.X`, `&numbers[0]` legal; `&(count + 1)` and `&Point().X` illegal |
| 11 | 5331–5379 | 4.9.26 | `Optional` fallback, illegal bare `?? throw`, `?? throw MyException()` in `trap`, `Result` `?? throw` / `?? 0` / `?? return` |
| 12 | 5416–5442 | 4.9.27 | `Max` via `a > b ? a : b`, sign extraction; illegal non-boolean condition, no common branch type |

---

## 7. Open questions (ambiguities / gaps / inconsistencies)

**OQ-1 (contradiction).** L3811 says accessor members MUST NOT be accessed through a member-access expression, while L3817 says when the member is an accessor the accessor is read and its read result is the member-access result. *Proposed interpretation:* L3811 forbids taking the accessor **declaration** as a value (e.g. passing `obj.Prop` as a function object); normal property reads go through `get` per L3817. Codegen needs both behaviors distinguished at name resolution. Confirmed in range only; §3.8.2 not read.

**OQ-2 (evaluation duplication).** `a <= b` is specified as "substitute the `<` result as the left operand and the `==` result as the right operand of a logical-or" (L5090). A literal desugaring `a < b || a == b` evaluates `a` and `b` twice, and — because `||` short-circuits (L5272) — the `==` overload is not even called when `<` is true, so a side-effecting `==` overload never runs. This sits in tension with S-ORD-2 (L3626) and with L4627 ("when `<` is true **or** `==` is true"). *Proposed interpretation:* operands are evaluated once into temporaries; the `<` overload is called; the `==` overload is called only if `<` yielded `false` (short-circuit is intentional); result = `||`. Same for `>=` with `>`. Implementers should decide whether the equality overload's side effects are observable — the spec does not say. (See §8 for the lowering that keeps both readings possible.)

**OQ-3 (empty argument list).** L3922: "允许省略实参，当且仅当使用函数标识符" (arguments may be omitted iff a function identifier is used), but L3947 (`callback();`), L3979 (`Invoke(() => { });` has an argument, but the callee is a parameter), and L3993 (`(*functionPrinter)();`) show zero-argument calls through non-identifier callees. *Proposed interpretation:* an empty `Arguments` list is legal for any callee of function type; L3922 likely means a *bare function reference* (callee written without `()`) is allowed only for identifiers. Needs confirmation against §8.3.

**OQ-4 (bitwise result type).** L5174/L5198/L5222 allow the two operand types to differ when one is implicitly convertible to the other, but L5185/L5209/L5233 say the result type is "`Expression` 的返回值类型" — singular, with two `Expression`s in the production. *Proposed interpretation:* result type = the common type after the implicit conversion (the type both operands end up as); if one operand is a pointer, the pointer type. Note that if the "other" side is the converted one, the result type must be the non-converted side's type.

**OQ-5 (construction vs. call parse).** `ConstructionExpression` and `FunctionCallExpression` are both at precedence level 3 (L4515), and `FunctionCallExpression = Expression "(" Arguments ")"` can derive `Point(0,0)` from `ValueExpression`. Also `ConstructedTypeName "." Name "(" Expression ")"` overlaps `MemberAccessExpression` + call. L4134–4136 resolves the member-construction case by value-carrying-ness, and L4136 forbids parsing it as a call, but nothing states how `T(...)` is disambiguated from a call to a function named `T`. *Proposed interpretation:* resolve the head name in the type namespace first; if it names a type (or monomorphization) use construction/member-construction, otherwise call. Type and value namespaces are not specified in this range.

**OQ-6 (implicit conversions for arithmetic/comparison overloads).** §4.9.8–4.9.12 and 4.9.15–4.9.20 only require "a matching overload for the left and right expression types" (L4877 etc.); they do not say whether implicit conversions participate (unlike bitwise, which explicitly allows one side to convert, L5174). §8.5 (out of range) presumably defines it. *Proposed interpretation:* defer to §8.5; if it allows conversions, the callee's parameter types determine the converted operand types; otherwise exact match is required.

**OQ-7 (cast resolution scope).** L3698 says "prefer the conversion operator that accepts the converted value", but does not say whether the operator is looked up on the **source** type (extension of the source type targeting `Type`) or the **target** type. The example declares the operator in `RGBColor`'s extension with a `from: HSLColor` parameter (L3736–3744) — i.e. on the **target** type. *Proposed interpretation:* both directions are searched; prefer the operator whose declaring type is the cast target (as in the example), fall back to one on the source type; ambiguity is resolved by §8.5. Not stated in range.

**OQ-8 (call evaluation order).** §4 states only sibling left-to-right (L3626), and a call's callee plus its arguments are siblings syntactically. Named arguments are written in an arbitrary order (L3962). *Proposed interpretation:* evaluate callee first, then arguments in **written** order (not parameter order); named-argument reordering happens in the callee's parameter mapping only.

**OQ-9 (`<=` / `>=` NaN requirement).** L4629 requires IEEE 754 and L4631 states `<=` must be false for NaN, but the desugaring `< || ==` naturally yields false for NaN only for builtin float comparisons. For **user-defined** `<`/`==` returning `boolean`, NaN handling is entirely the user's responsibility; the spec does not say the compiler must enforce IEEE semantics for user overloads. *Proposed interpretation:* builtin float comparisons are ordered (`fcmp olt`/`oeq`), user overloads are trusted.

**OQ-10 (`??` laziness vs. sibling order).** The `CoalescingAlternative` is syntactically a subexpression of the `??` expression, so S-ORD-1/2 (L3625–3626) would demand it be evaluated; L5302/L5304 require it not to be (Value case) or to be lazy. *Proposed interpretation:* §4.9.26's specific rule overrides §4's general order (as L3628 already implies for `??`).

**OQ-11 (boolean requirement for `<`/`>`/`==`).** L4398 says the operator's return type is the result type (no boolean constraint), and L5044/L5066/L5138 simply yield the overload's return value. But `<=`, `>=`, `!=` feed those results into `||`/`!`, which require `boolean` (L5246, L5273, L4755). Therefore an overload returning non-boolean is usable directly but makes the derived operators ill-typed. *Proposed interpretation:* do not restrict declarations; report a type error only at the derived-operator use site, as the desugaring text implies.

**OQ-12 (`is` with a non-lvalue / non-reference left operand).** Any `Expression` is allowed as the left operand (L4230); no lvalue or pointer requirement is stated, and the example tests a literal `1 is Cat` (L4321). *Proposed interpretation:* `is` takes the value (or its static type for the impossible case), never requires addressability; class/interface tests need the runtime type descriptor of the value's dynamic type (see §8).

**OQ-13 (`-infinite` tokenization).** `InfinityTarget = [ "-" ] "infinite"` (L4236) makes `-` part of the **target**, not a `NegateExpression`; but `-infinite` is also legal as a standalone expression (`local lower: float(4) = -infinite;`, L4736) where it *is* the negate operator applied to the `infinite` literal (or a signed literal). *Proposed interpretation:* in `is` context, parse `-` as part of the target; elsewhere, unary negate. Whitespace is not constrained by the EBNF (OQ-14).

**OQ-14 (whitespace in multi-token targets).** `"infinite"`, `"illegal"`, `Cast` and `is` are word tokens; the spec does not state whether `- infinite` (space) is a valid `InfinityTarget`, nor whether `Cast <T> (x)` is valid. *Proposed interpretation:* require the exact adjacency the examples show (`(x as T)`, `-infinite`, `a is B`); reject whitespace inside `Cast<...>` but allow it around `is` and after `-` — unspecified.

**OQ-15 (index accessor applicability / array-with-indexer).** L3871 says the source type must be an array **or** carry an index accessor; L3876 says array → builtin. If a class/struct is an array type and also declares an indexer (not shown in range), the array builtin wins by L3876. *Proposed interpretation:* arrays can never have user indexers; indexers are only for class/structure types. Also: indexer parameter types are arbitrary (L4400), but the index expression type is constrained to cardinal/convertible (L3867) — that constraint should apply only to **builtin array** indexing; for indexers, applicability should be ordinary overload resolution. This is a real inconsistency: L3867 is stated unconditionally, yet L4400 allows any index parameter types.

**OQ-16 (shift semantics).** `>>` is named "logical" right shift (L4974) but the spec does not define signedness behavior, shift-amount masking, or what happens when the amount ≥ bit width. *Proposed interpretation:* logical (zero-fill) shift regardless of the integer's signedness (use `lshr`); the amount domain is the right operand's cardinal value; behavior for out-of-range amounts is per §3.2. Also `<<` is named "logical left shift", so `shl` is fine.

**OQ-17 (integer overflow / division by zero).** §4.9.8–4.9.10 delegate to the type pages (§3.2) and say nothing about wrapping, trapping, or poison for `/` by zero, `%` by zero, or `INT_MIN / -1`. *Proposed interpretation:* follow the integer type page; if it is silent, choose defined wrapping for `+ - *`, and a defined trap/exception for division by zero — but this must be confirmed before codegen fixes flags.

**OQ-18 (member access on a module).** L3792 allows a module as source and L3797 requires the member to be a module declaration, but `AccessExpression` is described as accessing identifiers/instances and the `MemberAccessExpression` production's source alternatives are `Identifier | Expression`; a module name is presumably an `Identifier`, but the type rules call it a distinct source kind. *Proposed interpretation:* modules are named by identifiers and resolved in the module namespace; no codegen emission.

**OQ-19 (index of a `verbatim` block).** L3895–3896 requires removing bounds checks when provably in-bounds "except when the index is in a `verbatim` block". This means checks are **retained** in verbatim blocks even when provably safe (opposite of an optimization opt-out), which is unusual. *Proposed interpretation:* in `verbatim`, emit the check always; outside, elide when provable. Confirm with §9.8.

**OQ-20 (`Result<; T>` empty-type spelling).** L5321 uses `Result<; T>` and `Result<T;>` with an omitted generic argument; the generic-argument grammar is §10/§15 (outside range). Also `Result<TValue; TAlternative>` uses `;` as the separator, while `Cast<Type>` and `Optional<T>` use a single argument. *Proposed interpretation:* `;`-separated parameter lists with optional omission producing an "empty type" argument; this must match §15.2 before parser work.

**OQ-21 (associativity vs. same-level rule).** L4535 says same-level operators are left-associative, then L4536 carves out `??` and `?:` as right-associative. Level 8 mixes `<`, `>`, `<=`, `>=`, `is` — all left-associative, e.g. `a < b < c` parses `(a < b) < c`, which is a type error for builtin booleans. No chaining restriction is stated. *Proposed interpretation:* parse left-associatively and let the type checker reject.

**OQ-22 (operator definition inside an interface).** L4378 exempts "contract operators in interface declaration bodies" from the extension-block requirement, and L4414 says interface-body indexers are contracts declared directly; but the body form (`:` only vs. accessor members) is only partly specified (L4377 requires `:` or `=`). *Proposed interpretation:* interface-body operators are declarations only (`:` form), no `=`/`Block`; confirm with §3.7.3.

---

## 8. LLVM 18 codegen implications

> Derived from the rules above; points that depend on unresolved questions are marked with the OQ id. No new language rules are introduced here.

### 8.1 Operator lowering and dispatch

- Resolve every operator at compile time (S-OVL-2/S-OVL-8, L4612, L4637). Emit either an IR instruction (built-in overload, S-OVL-10) or a direct call to the mangled operator function (user overload). Unary vs. binary is chosen by the definition's parameter count (L4636); the operator symbol is the overload name for §8.5 lookup (L4635). Because built-in and user overloads are treated identically by dispatch (L4652), the front end should produce a single `OperatorRef` node (builtin-kind | function-symbol) before lowering.
- Mangling must encode the symbol plus the resolved operand types; conversion operators and indexers need their own mangling (target type / parameter list) since they are not `OperatorSymbol`s (L4346–4350).
- Built-in integer `+ - * / %`: `add/sub/mul/sdiv/udiv/srem/urem` — signedness from the operand type (integer vs cardinal). Integer overflow and division by zero are OQ-17; until resolved, do **not** attach `nsw`/`nuw` flags (they would assume undefined behavior the spec has not stated) and do not attach fast-math flags to float ops (IEEE 754 is mandated for comparisons at L4629; keep strict FP semantics).
- Float `+ - * /`: `fadd/fsub/fmul/fdiv` with default (strict) fast-math (none). `%` on floats: `frem` only if the float type page defines it; otherwise the overload must exist or it is a type error (S-ARITH-1).
- `-` (negate): builtin integer `sub 0, x`; float `fneg`; otherwise call the unary `-` overload (S-NEG-1). There is no unary `+` in `Expression` (L3544–3579).
- `!`: convert the operand to `i1` per S-IMP-3, then `xor i1 %b, true` (or `icmp eq i1 %b, false`). Not overloadable (S-OVL-4).
- Unary `^`: integer/cardinal `xor x, -1`; pointer → `ptrtoint`, `xor` with all-ones of the pointer width, `inttoptr` (S-BNOT-1 + L4780).
- Binary `&`/`^`/`|`: `and/or/xor` at the common type; pointer operands go through `ptrtoint`/`inttoptr` and the operation applies to the pointer value itself (L5180, L5204, L5228). Result type = common type (OQ-4).
- Shifts: `shl` for `<<`; `lshr` for `>>` (named **logical**, L4974; OQ-16). Amount: convert the right operand to the left operand's bit width (`zext`/`trunc`), since it is cardinal (S-SH-1); pointer left operand → `ptrtoint`, shift, `inttoptr` (L4991). Out-of-range amounts are OQ-16; if the integer page does not define them, either mask the amount or emit a guard, but do not rely on LLVM's poison semantics silently.
- `&` reference: emit the address, not a load. Addressable forms (S-REF-2): alloca/global/parameter slot (identifier); dereference → pass the pointer through unchanged; field → `getelementptr` into the base address (base must itself be an addressable lvalue); built-in index → `getelementptr` with the bounds check (S-IDX-6). A temporary has no address → front-end error (L4834–4839).
- Unary `*`: emit `load` from the pointer (S-DER-2). As an assignment target, keep the pointer and emit `store` at the assignment site instead of materializing a value; LLVM `load` produces an SSA value, so lvalue contexts must be represented as addresses, not loaded values.

### 8.2 Comparisons, equality, derived operators

- `<`, `>`: builtin integer → `icmp slt/sgt` (integer) or `icmp ult/ugt` (cardinal); float → **ordered** `fcmp olt/ogt` so NaN yields false (L4629). User overload → call (S-CMP-1).
- `==`: builtin integer → `icmp eq`; float → `fcmp oeq` (NaN ≠ NaN); user overload → call.
- `!=`: desugar to logical-not of `==` (S-CMP-5): call/emit `==`, then invert (`xor i1 %r, true`; for builtin float `fcmp une` is equivalent and is the cleanest single instruction, matching IEEE inequality L4629). `!=` MUST NOT be defined by the user (L4625).
- `<=`: evaluate the two operands **once** into temporaries (OQ-2), call `<`; branch: if `true` → result `true`; if `false` → call `==`; result is the OR. This preserves the short-circuit implied by L5090 + L5272 while avoiding double evaluation. If OQ-2 is resolved to require evaluating both unconditionally, drop the branch and OR the two calls. `>=` mirrors with `>`.
- Important: with user overloads the `==` call may have side effects; the branch form makes those side effects conditional. Flag this in the implementation notes and keep the desugaring switchable.
- NaN guarantee: because `<=` is not `!(>)` (L4630–4631), do not implement `<=` as `fcmp ule`? — actually `fcmp ule` is false for NaN, so it is semantically correct for builtins; the prohibition is about definition/desugaring, not about the instruction. Builtin float `<=` may lower to `fcmp ole` directly (ordered, false on NaN), which is equivalent to `< || ==` for non-NaN and matches L4631.

### 8.3 Short-circuit `&&`, `||`

- Convert each operand to `i1` (S-IMP-3) **at its use point**, not eagerly for the RHS.
- `a && b`: evaluate `a` → `%a1 = trunc/… to i1`; `br i1 %a1, label %rhs, label %done_false`; `rhs:` evaluate `b`, convert to `i1`, `br label %done`; `done_false:` false. Merge with a `phi i1` at `%done` (or with an alloca/select if the front end cannot build phis cleanly). Never emit RHS code in a block dominated by the `false` path (L5252).
- `a || b`: mirror with `br i1 %a1, label %done_true, label %rhs` (L5272).
- `!`, `&&`, `||`, `?:` are not overloadable (S-OVL-4), so no user calls are involved; only implicit boolean conversions.
- Do not use `select` for `&&`/`||`/`?:`/`??` when the RHS has side effects: `select` evaluates both sides. Use branches + phi. (LLVM `select` is not short-circuiting.)

### 8.4 Conditional `?:`

- Evaluate the condition, convert to `i1`, branch (S-COND-4). Emit exactly one arm; each arm converts to the common type (S-IMP-5, S-COND-3); merge with a `phi` of the common type.
- If both arms are the same SSA type, a `select` is legal **only** when both arms are side-effect-free; otherwise branch+phi.
- Common-type rule includes the derived-type case (one branch type derives from the other → base type, L5402). This is a front-end coercion of the derived value to the base (reference upcast), not an LLVM bitcast for aggregates — represent classes as pointers/refs so the coercion is a no-op or a GEP.
- Not overloadable (L5412).

### 8.5 Null-coalescing `??`

- Evaluate the left operand **exactly once** (S-NC-1) into a temporary/SSA value; call the left type's `operator ??` (a normal function returning `Result<TValue; TAlternative>`) — this is the only permitted lowering; `??` cannot be an ordinary binary operator call (L5326–5327).
- `Result` layout: `{ tag: i1/i8, value: TValue, alternative: TAlternative }` (or a union for the payload). For `Result<T;>` / `Result<; T>` the corresponding payload is an **empty type** (S-NC-6): emit no storage (or an `i8` placeholder) but keep the tag; never load/store the empty payload.
- Lower: `%r = call @op_coalesce(%left)`; `%isValue = extractvalue %r, 0`; `br i1 %isValue, label %value, label %alternative`.
  - `%value:` `%v = extractvalue %r, 1`; branch to merge; `%phi` gets `%v`.
  - `%alternative:` evaluate the continuation **lazily** (S-NC-2 / L5302); then merge.
- Continuation forms (S-NC-3):
  - `Expression` → evaluate here, convert to `TValue` if needed (its type MUST be `TValue`, L5304), merge.
  - `throw` → load the alternative payload `a` and raise it; if `TAlternative` is empty, this form is a compile error (L5305, L5322) so codegen never sees it.
  - `throw Expression` → evaluate `Expression` and raise it; do **not** use `a` (L5306). Position restrictions of §5.10.3 apply.
  - `return` → return the entire `Result` value from the current function; the function's return type MUST equal that `Result` type (L5307). This is a terminator in the alternative block; the merge block is then only reachable from `%value` (or the continuation may be `unreachable` on that path).
  - `return Expression` → return that expression (converted to the function's return type, L5308); terminator likewise.
- Right-associativity (L4536) means `a ?? b ?? c` groups as `a ?? (b ?? c)`; the alternative `b ?? c` is itself a `NullCoalescingExpression` and is only evaluated on the Alternative path (nested laziness).
- Do not mark the left call `readnone`/`speculatable`: it must execute exactly once and its decision is observable.

### 8.6 Calls

- Evaluate the callee subexpression first, then arguments in written order (S-ORD-2, OQ-8). Named arguments are reordered into the callee's parameter order at the call site, not evaluated in parameter order.
- Direct call for a known function symbol; otherwise indirect `call` through the function-pointer value (S-CALL-9).
- Member calls pass the receiver as the explicit `this` argument; the spec's method examples write `this` explicitly in constructors/accessors (L4068, L4416) but ordinary methods use `this` implicitly (S-MEM-6). Keep one ABI: receiver as first pointer parameter.
- Reference parameters (e.g. `Scale(value: &integer(4), ...)`, L3939) pass an address; mutation through the parameter is a `store` to that address. The `&` at the call site is implicit for reference parameters (example L3969 passes `value`, not `&value`).
- Default arguments are filled at the call site (S-CALL-5, example L3956); missing required argument is a front-end error (L3999–4001). Duplicate/named-after-positional are front-end errors (L3981–3983, L3995–3997).
- Zero-argument calls and immediately-invoked function literals lower as ordinary indirect calls (examples L3971–3976, L3993). OQ-3 must be settled before deciding whether `f` without `()` is a value reference.
- `sret` for aggregates: return the constructed/call result through a hidden destination pointer when the type is not trivially returned; this dovetails with in-place construction (8.7).

### 8.7 Construction and member construction

- On the RHS of a definition/assignment, pass the **destination address** as the constructor's `this` and call the constructor directly; MUST NOT allocate a temporary and copy (S-NEW-6, L4044–4045). For `local origin: Point = Point(0,0);` (L4086) this is: `%origin = alloca %Point; call @Point_Construct(%origin, 0, 0)`.
- Off-RHS, allocate storage first (compiler-chosen, §12.1; stack `alloca` or heap per the type's allocation rules) and construct into it (S-NEW-7).
- Classes are reference types in the examples: `local cat: Cat = Cat();` constructs a `Cat` object (L4089). Represent class values as pointers; the construction expression yields the pointer. The allocation policy is §12.1 (out of range) — do not guess.
- Member initialization per the type's lifetime MUST run before the constructor body (S-NEW-8, L4051); emit field initializers in declaration order first, then call the constructor.
- No allocation specifier is allowed on construction/member construction (L4043, L4140); a `heap` keyword there is a parse/type error (L4105–4107).
- Member construction for a **variant**: store the tag for `Name` and store the payload value of `Expression` (S-MCT-5, L4147). For a **union**: `getelementptr i8, %Union*, 0, 0` (or a bitcast of the union's address) and store the member value at offset 0; do not initialize any other member (S-MCT-6, L4148). Do not zero the rest of the union storage — the spec explicitly forbids initializing the other members.
- Payload-less variant states are plain member accesses (S-MCT-3, L4207), not constructions; `Status.Done()` is a front-end error (L4212–4214).
- `Pixel.RGBA(Color(1,2,3,4))` demonstrates nesting: construct the `Color` argument (possibly into a temporary argument slot) and pass it into the union member store (L4197). Heap+union (`heap heapPixel: Pixel = Pixel.RGBA(...)`, L4200) means the union storage comes from the heap and the member store targets it.

### 8.8 `is` expressions

- Result is always `boolean` → `i1` (S-IS-2). If the static type makes the test impossible, fold to `false` (S-IS-9, L4321) — no runtime work.
- Class target: compare the value's runtime type identity against the target class or its derived types (S-IS-4). The spec does not define an RTTI representation in this range (OQ-12); options: a type-id word in the object header, or a `typeid` pointer compared via a precomputed "is-a" set. Emit a call to a runtime helper if the hierarchy is open.
- Interface target: test whether the runtime type's interface table contains the target interface (S-IS-5); typical lowering is a runtime lookup over an itable/interface map. Also undefined by this range.
- Variant member: load the variant tag and `icmp eq` with the member's tag constant (S-IS-6).
- `illegal`: float NaN test → `fcmp uno %x, %x` (true iff unordered). Only valid for float types (S-IS-3); other types are a front-end error (L4330–4332).
- `infinite`: `fcmp oeq %x, 0x7FF0000000000000` for +inf / `0xFFF0000000000000` for −inf (using the operand's float width); the sign must match (S-IS-8). For `-infinite` the comparison is against −inf; do not implement as `fabs(x) == inf`.
- The left operand is a value, not necessarily an lvalue (OQ-12); no address is required. For class/interface tests the value is the object reference; loading its type descriptor is a runtime operation.
- `is` MUST NOT be affected by operator overloading (S-IS-10) — never route through a user `==`.

### 8.9 Casts

- Resolution order at codegen (S-CVT-3): (1) user conversion operator accepting the value → direct call, result type = target type (L4401–4402); (2) otherwise builtin explicit conversions from the type pages (L3699).
- Builtin explicit conversions to expect: integer width narrowing/widening (`trunc`, `sext` for integer, `zext` for cardinal), bool→numeric (`zext i1`), int↔float (`sitofp`/`uitofp`, `fptosi`/`fptoui`), float width (`fptrunc`/`fpext`), pointer↔integer (`ptrtoint`/`inttoptr`), pointer↔pointer (`bitcast`/`addrspacecast`), class upcast/downcast (front-end-checked, usually no instruction). An explicit conversion that does not exist is a compile error (L3700–3701).
- `implicitable` conversion operators may be inserted implicitly at mismatch sites (S-CVT-5, L3706) — the front end must mark them so the lowering can distinguish an explicit `Cast<T>` from an implicit insertion; both lower to the same call.
- `Cast<T>` never runs a constructor and never allocates; it produces an `T` value (or an in-place conversion if a user operator does).
- Float→int conversion of NaN/infinity is undefined in LLVM (`fptosi` poison). The spec does not state the intended behavior; if the type page requires a defined result, guard with `fcmp` before conversion (OQ-17-adjacent; flag for the cast/type-page owner).
- Bool→numeric is explicitly legal (example L3722 yields `1`), so `zext i1` is the right lowering.

### 8.10 Bounds checks and `verbatim`

- Built-in array indexing (S-IDX-3) must check `index >= size` (unsigned compare, since the index is cardinal) and branch to a throw path that constructs/raises `OutOfRangeException` (S-IDX-6, L3893). Use `icmp uge iN %idx, %len`; the array length is part of the array type (§3.6.1) so it is a constant for fixed-size arrays.
- If the compiler proves `idx < len` statically, remove the check — **except** inside `verbatim` blocks, where the check must remain (S-IDX-8, L3895–3896; OQ-19).
- If the compiler proves `idx >= len` statically, emit a warning (S-IDX-7, L3894) and may keep the check/throw (or fold to the throw path).
- Indexer (non-array) access performs no built-in bounds check; it calls `get`/`set` (S-IDX-4/S-IDX-5). Assignment calls `set(this, index, value)` (L3887); read calls `get` (L3877). The `get`/`set` bodies are accessor members with explicit `this` (L4416).

### 8.11 General IR hygiene

- Preserve written left-to-right order for sibling subexpressions (S-ORD-2): emit instructions in that order and do not let optimization flags imply otherwise. Do not attach `reassoc`/`contract`/`afn`/`nnan`/`ninf` to arithmetic (IEEE 754 is mandated at L4629 and `is` must observe NaN/infinity exactly).
- Memory: loads/stores of fields and array elements must respect the lvalue rules (8.1, 8.7); do not forward a store across a call that may alias unless the callee is marked accordingly — the spec does not define aliasing rules for user operators, so be conservative.
- Trap/throw: `OutOfRangeException` (L3893) and the `??` `throw` continuations (L5305–5306) need the §5.10 exception machinery; codegen must route them through the same landing-pad/unwind path as `throw` statements, and `return` continuations (L5307–5308) through the function's normal return with the correct return type.
- Operator calls are ordinary function calls for ABI purposes (S-OVL-8), so inlining is optional; no special calling convention is implied by this range.

---

## 9. Quick implementation checklist (traceability)

1. Parse `Expression` as the 34-alternative sum type; the 16-level precedence climber with postfix > unary > binary, left-assoc except `??`/`?:` right-assoc (S-PREC-1..3).
2. Disambiguate homographs by position (S-PREC-3).
3. Name-resolve member access with the 3-step member order and the static/type-source rules (S-MEM-1..9).
4. Type-check cast via conversion-operator-first rule (S-CVT-3..7).
5. Type-check calls/ctors via §8.5 overload resolution; enforce declared-type rules (S-CALL-1..4, S-NEW-2..4, S-MCT-1..2).
6. Desugar `!=`, `<=`, `>=`; keep operands single-evaluated (S-CMP-2..7, OQ-2).
7. Enforce the 12-symbol overloadable set and derived-operator ban (S-OVL-1..9).
8. Enforce addressability for `&` (S-REF-1..3), pointer rules for `->`, `*`, shifts, bitwise (S-PTR, S-DER, S-SH, S-BIT).
9. Type-check `??` (Result protocol, continuations, empty types) and `?:` (common type) (S-NC-1..8, S-COND-1..5).
10. Emit short-circuit control flow only for `&&`, `||`, `?:`, `??`; everything else in written order (S-ORD-2, 8.3–8.5).
11. Emit in-place construction on RHS; no temp+copy (S-NEW-6, S-MCT-4).
12. Bounds checks with the `verbatim` exception (S-IDX-6..8).
