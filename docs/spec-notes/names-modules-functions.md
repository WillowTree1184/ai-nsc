# NovaSharp Draft 1.1 — 名称 / 模块 / 函数 / 修饰符 / 泛型

**Extraction target**: `docs/grammar/Draft 1.1.md`, spec lines **7215–8952** (inclusive):
§6 名称 (6.1–6.4), §7 模块, §8 函数 (8.1–8.5), §9 修饰符 (9–9.15), §10 泛型 (10.1–10.2).

**Rules of this note**

- EBNF in §2 is copied **verbatim** from the spec, in source order, one block per heading.
- Every normative bullet is tagged with the spec line range it came from (`[L7219–7243]`).
- Nothing is invented: items the spec leaves undefined are listed in §6 **Open questions** with a proposed interpretation and a line anchor.
- Cross-references that point **outside** 7215–8952 are marked `XREF` and are *not* treated as normative here (anchors listed in §1.2). Where the range depends on them (e.g. `Block`, `FunctionTypeConstructor`, `use`, `OperatorSymbol`), the dependency is called out.

## 1. Section map

### 1.1 In-range anchors

| Lines | Section | Normative payload |
| --- | --- | --- |
| 7215–7327 | **§6 名称** | scope taxonomy, accessibility, identifier uniqueness, shadowing + `owner` |
| 7217–7243 | §6.1 作用域 | six scope forms; file/module scope definitions; inner/outer access; `#{}` is not a scope |
| 7245–7256 | §6.2 可访问性 | default accessibility delegated to §9.1; modifiers override; restricted identifiers invisible to outer scopes |
| 7258–7278 | §6.3 标识符唯一性 | no same-name in one scope; three exception families; definition order irrelevant |
| 7280–7327 | §6.4 遮蔽 | shadowing predicate; `owner` escape; nesting of `owner`; worked example (7302–7327) |
| 7329–7375 | **§7 模块** | `module` declaration, module scope, no type use, nesting, type-body modules have no runtime entity; example 7359–7375 |
| 7377–7490 | **§8 函数 / §8.1 函数定义语法糖** | `fixed` + `Block` sugar, expansion rules, `this` explicitness; example 7421–7490 |
| 7492–7594 | §8.2 参数 | `Parameters` / `LiteralParameters`, `#`, `&`, defaults, `this`; example 7560–7594 |
| 7596–7661 | §8.3 实参 | `Arguments`, named args, substitution, omission of defaulted args; example 7636–7661 |
| 7663–7785 | §8.4 方法 | method predicate, `this` parameter rules, call restrictions, layout exclusion; example 7704–7785 |
| 7787–7870 | §8.5 重载决议 | exact-match / convertible-match algorithm, signature rules, calling-convention rules; example 7830–7870 |
| 7872–7958 | **§9 修饰符** (framework) | modifier categories EBNF, placement, ordering, no duplicates, target legality; example 7941–7958 |
| 7960–8048 | §9.1 可访问性修饰符 | base vs trim dimensions, intersection, defaults, per-scope non-differentiation, function-value access rights; example 8022–8048 |
| 8050–8095 | §9.2 `public` | keyword, "accessible by all scopes"; example 8075–8095 |
| 8097–8155 | §9.3 `private` | access set incl. type scope and `this`-carrying functions, extension-block narrowing; example 8125–8155 |
| 8157–8217 | §9.4 `protected` | access set = declaring scope + derived scopes; example 8183–8217 |
| 8219–8368 | §9.5 `scoped` | trim targets (`Identifier`, `sealed`, `operator`), self/inheritance items, union/intersection; example 8269–8368 |
| 8370–8385 | §9.6 `readonly` | field-only, no `fixed`, init/ctor assignment only |
| 8387–8461 | §9.7 `implicitable` | conversion operators only, implicit callability, lossy allowed; example 8410–8461 |
| 8463–8508 | §9.8 `verbatim` | block/variable/constant targets, no optimization, no compile-time evaluation; example 8495–8508 |
| 8510–8525 | §9.9 `manual` | `heap` only, no NOLAA release, explicit `dispose` |
| 8527–8543 | §9.10 `abstract` | class-only, not instantiable, ctor/dtor inheritance |
| 8545–8559 | §9.11 `sealed` | class declaration + override method |
| 8561–8575 | §9.12 `virtual` | method-only, no `structure`, body required |
| 8577–8591 | §9.13 `override` | method-only, class override requirement, covariant `this` |
| 8593–8649 | §9.14 `aligned(N)` | struct default alignment, field alignment, union member offset, software emulation; example 8628–8649 |
| 8651–8780 | §9.15 `static` | allowed/forbidden targets, layout, access path, per-instantiation attachment, static ctor/class/accessors; example 8704–8780 |
| 8782–8914 | **§10 泛型 / §10.1 泛型参数** | `GenericParameters`, constraints, allowed positions, monomorphization requirement; example 8855–8914 |
| 8916–8949 | §10.2 单态化表达式 | `MonomorphizationExpression`, arity/constraint checks, compile-time semantics |
| 8950–8951 | boundary | blank line, start of §11 编译期 (out of range) |

Gaps *inside* the range worth knowing up front: the `有关初始化：` heading at **L7417** is empty; §7 has **no** import/qualification grammar (imports live in §5.6, out of range); §9.10 has no `abstract` *method* form.

### 1.2 Out-of-range anchors referenced by the range (XREF)

| Anchor | Section | Why it matters here |
| --- | --- | --- |
| L325 | 2.4 标识符 | `Name` / `Identifier` used throughout §7–§10 |
| L400 | 2.5 标识符定义简写 | function shorthand expansion (L7403, L7526) |
| L511, L529, L531 | 2.6 关键字 | `this`, `owner`, `base`, `use`, `Cast`, `basic`, `wildcard`, `extension` |
| L1036 | 2.7.9 函数 | function literal parameters (L7525) |
| L1458 | 3.1 类型构造语法 | `Type` |
| L1508 | 3.2 基本类型 | `basic` kind |
| L1663 | 3.5 引用 | `&` reference semantics (L7549) |
| L1713, L1913 | 3.6 / 3.6.3 函数类型构造器 | `FunctionTypeConstructor`, calling convention, parameter types |
| L2082 | 3.7.1 类 | override requirements (L7273, L8589, L8591), ctor/dtor |
| L3097 | 3.8.1 字段 | `FieldDefinition` (L8382, L8665) — field syntax has **no** `fixed` |
| L3176 | 3.8.2 访问器 | type-level accessors (L8667, L8692–8695) |
| L3414 | 3.8.3 扩展 | extension blocks (L7671, L7679) |
| L3678 | 4.3 类型转换表达式 | implicit conversion admissibility (L7622, L7805, L8404) |
| L3773 | 4.4.1 成员访问表达式 | `owner.`, method/static access (L7296, L7690, L8677) |
| L4335 | 4.9 运算符 | `OperatorSymbol`, operator overload dispatch (L7824, L8257–8261) |
| L5530 | 5.1 代码块 | `Block` |
| L5583, L5663, L5754, L5859 | 5.2 `local`, 5.3 `fixed`, 5.4 `heap`, 5.5 `linkable` | targets of §8/§9 |
| L5938–5987 | 5.6 `use` 语句 | the actual import mechanism; **not** in this range |
| L6107 | 5.8 `dispose` | manual release (L8525) |
| L9158, L9302 | 11.5 编译期常量参数, 11.7 编译期边界 | `#` parameters, compile-time bindings |
| L9679, L9719 | 14.1 主函数, 14.2 启动流程 | static ctor before `Main`; static field init; section placement |
| — | 14.3.2 体积优化 | monomorphization volume optimization (L8949) — anchor not located |

## 2. Consolidated EBNF (verbatim, source order)

### 2.1 §7 模块 — `ModuleDeclaration` (L7333–7335)

```ebnf
ModuleDeclaration = { Modifier } "module" Name Block .
```

### 2.2 §8.1 函数定义语法糖 — `FunctionDeclarationSugar` (L7383–7385)

```ebnf
FunctionDeclarationSugar = { Modifier } "fixed" Name [ GenericParameters ] ":" FunctionTypeConstructor Block .
```

### 2.3 §8.2 参数 — `Parameters`, `Parameter`, `LiteralParameters`, `LiteralParameter` (L7496–7504)

```ebnf
Parameters        = Parameter { "," Parameter } .

Parameter         = [ "#" ] [ Name ":" ] [ "&" ] Type [ "=" Expression ] .

LiteralParameters = LiteralParameter { "," LiteralParameter } .

LiteralParameter  = [ "#" ] Name [ ":" [ "&" ] Type ] .
```

### 2.4 §8.3 实参 — `Arguments`, `Argument` (L7600–7604)

```ebnf
Arguments = [ Argument { "," Argument } ] .

Argument = [ Name ":" ] Expression .
```

### 2.5 §9 修饰符 — `Modifier` and categories (L7876–7896)

```ebnf
Modifier = AccessibilityModifier
         | BehaviorLimitingModifier
         | BehaviorEnablingModifier
         | OptimizationModifier
         | OtherModifier .

BehaviorLimitingModifier = ReadonlyModifier .

BehaviorEnablingModifier = ImplicitableModifier .

OptimizationModifier = VerbatimModifier .

OtherModifier = ManualModifier
              | AbstractModifier
              | SealedModifier
              | VirtualModifier
              | OverrideModifier
              | AlignedModifier
              | StaticModifier .
```

### 2.6 §9.1 可访问性修饰符 — `AccessibilityModifier`, `BaseAccessibilityModifier` (L7964–7970)

```ebnf
AccessibilityModifier = BaseAccessibilityModifier | ScopedModifier .

BaseAccessibilityModifier = PublicAccessibilityModifier
                          | PrivateAccessibilityModifier
                          | ProtectedAccessibilityModifier .
```

### 2.7 §9.2 `public` — `PublicAccessibilityModifier` (L8054–8056)

```ebnf
PublicAccessibilityModifier = "public" .
```

### 2.8 §9.3 `private` — `PrivateAccessibilityModifier` (L8101–8103)

```ebnf
PrivateAccessibilityModifier = "private" .
```

### 2.9 §9.4 `protected` — `ProtectedAccessibilityModifier` (L8161–8163)

```ebnf
ProtectedAccessibilityModifier = "protected" .
```

### 2.10 §9.5 `scoped` — `ScopedModifier`, `ScopedTarget` (L8223–8228)

```ebnf
ScopedModifier = "scoped" "(" ScopedTarget { "," ScopedTarget } ")" .

ScopedTarget = [ "sealed" ] Identifier
             | "operator" [ Identifier | OperatorSymbol ] .
```

### 2.11 §9.6 `readonly` — `ReadonlyModifier` (L8374–8376)

```ebnf
ReadonlyModifier = "readonly" .
```

### 2.12 §9.7 `implicitable` — `ImplicitableModifier` (L8391–8393)

```ebnf
ImplicitableModifier = "implicitable" .
```

### 2.13 §9.8 `verbatim` — `VerbatimModifier` (L8467–8469)

```ebnf
VerbatimModifier = "verbatim" .
```

### 2.14 §9.9 `manual` — `ManualModifier` (L8514–8516)

```ebnf
ManualModifier = "manual" .
```

### 2.15 §9.10 `abstract` — `AbstractModifier` (L8531–8533)

```ebnf
AbstractModifier = "abstract" .
```

### 2.16 §9.11 `sealed` — `SealedModifier` (L8549–8551)

```ebnf
SealedModifier = "sealed" .
```

### 2.17 §9.12 `virtual` — `VirtualModifier` (L8565–8567)

```ebnf
VirtualModifier = "virtual" .
```

### 2.18 §9.13 `override` — `OverrideModifier` (L8581–8583)

```ebnf
OverrideModifier = "override" .
```

### 2.19 §9.14 `aligned(N)` — `AlignedModifier` (L8597–8599)

```ebnf
AlignedModifier = "aligned" "(" IntegerLiteral ")" .
```

### 2.20 §9.15 `static` — `StaticModifier` (L8655–8657)

```ebnf
StaticModifier = "static" .
```

### 2.21 §10.1 泛型参数 — `GenericParameters`, `GenericParameterDefinition`, `GenericParameter`, `Constraint`, `TypeKind` (L8788–8797)

```ebnf
GenericParameters = "<" GenericParameterDefinition { ";" GenericParameterDefinition } ">" .

GenericParameterDefinition = GenericParameter [ ":" Constraint { "," Constraint } ] .

GenericParameter = Identifier .
Constraint       = TypeKind | Identifier .

TypeKind = "class" | "structure" | "interface" | "enumeration" | "variant" | "union" | "basic" .
```

### 2.22 §10.2 单态化表达式 — `MonomorphizationExpression` (L8920–8922)

```ebnf
MonomorphizationExpression = ( Identifier | Expression ) "<" [ Type ] { ";" [ Type ] } ">" .
```

## 3. Semantic rules

### 3.1 Scopes (§6.1)

- **Definition** — a scope is the validity boundary of an identifier's occurrence `[L7219]`.
- Scope forms are exactly six: 全局作用域, 文件作用域, 模块作用域, 类型作用域, 函数作用域, 语句作用域 `[L7221–7228]`.
- **File scope** = the outermost scope of a source file `[L7232]`. **Module scope** = the scope formed by a module declaration body `[L7233]`.
- **Outer scope** = the scope directly containing the current scope; **inner scope** = a scope directly contained by it `[L7234–7235]`.
- If a source file contains **no** module declaration, all file content is in the **global scope** `[L7239]`.
- Inner scopes **MUST** be able to access identifiers of outer scopes `[L7240]`.
- Outer scopes **MUST NOT** access identifiers of inner scopes `[L7241]`.
- Module scopes **MAY** nest `[L7242]`.
- `#{ ... }` **MUST NOT** constitute a scope; declarations inside it **MUST** belong to the enclosing scope `[L7243]` (see XREF §11.4).
- Only 文件 and 模块 scope are *defined* here; 全局 is characterized at L7239; 类型 / 函数 / 语句 scope definitions live outside the range (XREF §3.7.1, §5.1).

### 3.2 Accessibility, general (§6.2)

- **Definition** — accessibility is the degree to which an identifier is visible to outer scopes `[L7247]`.
- An identifier's **default** accessibility **MUST** be determined by §9.1 `[L7251]`.
- Accessibility modifiers **MAY** override the default `[L7252]`.
- If an identifier is restricted by an accessibility modifier, outer scopes **MUST NOT** access it `[L7256]`.

### 3.3 Identifier uniqueness (§6.3)

- **Definition** — identifier uniqueness: no two same-name identifiers may occur in the same scope `[L7260]`.
- Same scope **MUST NOT** define same-name identifiers `[L7264]`.
- **Exception 1 — function constants**: same-scope same-name function constants are **allowed** when their parameter lists differ `[L7268]` (overloading, §8.5).
- **Exception 2 — methods**: same-name methods are allowed **iff** *either* the `this` parameter type differs *or* the parameter list after removing `this` differs `[L7269–7271]`.
- **Exception 3 — classes**: a derived class's identifiers **MAY** coexist with same-name base-class identifiers and **shadow** them `[L7272]`; overriding is governed by §3.7.1 `[L7273]`.
- Access to identifiers **MUST NOT** be constrained by definition order `[L7277]`. If the access occurs outside a function body, an earlier-defined identifier **MAY** access a later-defined one `[L7278]`.

### 3.4 Shadowing and `owner` (§6.4)

- **Definition** — shadowing: an inner-scope identifier makes an outer-scope same-name identifier inaccessible *by direct access* `[L7282]`.
- `X` shadows `Y` **iff all** of: same name; `X` defined in an inner scope; `Y` defined in an outer scope `[L7286–7289]`.
- If a name is shadowed, direct access to it in the inner scope resolves to the inner definition `[L7293]`.
- The keyword `owner` **MAY** be used to access a shadowed outer name `[L7294]`:
  - `owner` is immediately followed by `.` and the name `[L7295]`;
  - access to the shadowed name **MUST** go through a member access expression (XREF §4.4.1) `[L7296]`;
  - `owner` reaches names of the **directly** enclosing scope `[L7297]`;
  - `owner` **MAY** be nested; each nesting level moves one scope outward `[L7298]`.
- Worked example `[L7302–7327]`: global `a`; `Main`'s local `a` shadows it (`owner.a` = 1); a block-level `a: integer(8)` shadows `Main`'s `a`; `owner.owner.a` reaches the global `a`. Note the type change (`integer(8)` vs `integer(4)`) is irrelevant to shadowing.

### 3.5 Modules (§7)

- **Definition** — a module is a namespace that organizes identifiers `[L7331]`; syntax `ModuleDeclaration = { Modifier } "module" Name Block .` `[L7334]`.
- Modifiers **MUST** precede `module` `[L7346]`; the body **MUST** be a `Block` `[L7347]`.
- A module **MUST** constitute a module scope `[L7348]`; therefore its members live in a nested scope and are hidden from outer scopes by §3.2/§3.4.
- A module **MUST NOT** be used as a type `[L7349]`; example violation is commented out at L7372–7374.
- Module declarations **MAY** nest `[L7350]`; block-level modules are explicitly legal in the example `[L7361]`.
- Modules inside type declaration bodies (class / structure / interface) are **only** for identifier organization `[L7354]` and **MUST NOT** produce runtime entities `[L7355]`.
- Import mechanics: **not specified in this range.** `use` is a statement defined by §5.6 (XREF L5943: `UseStatement = "use" Identifier { "," Identifier } .`; L5953–5960: brings members of each target into the current scope, affects only later code in that scope, does not hoist). §11.2 (XREF L8982, L9011) adds that `use` never changes execution order and only affects name resolution. §7 itself gives no import/qualification production, no module-member access operator, and no rule that a module name and a type name share one namespace (see OQ2/OQ3).

### 3.6 Function definition sugar (§8.1)

- **Definition** — in a constant definition, a `Block` replaces `= <function literal>` `[L7381]`; syntax at `[L7384]`: `{ Modifier } "fixed" Name [ GenericParameters ] ":" FunctionTypeConstructor Block`.
- The **constant type MUST carry parameter names** `[L7397]` (subject to the §8.2 "useless name" exception, see OQ23).
- The sugar **MAY** be used for function members of all custom types `[L7398]`, and **MAY** be used for function members inside extension blocks `[L7401]`.
- The sugar applies **only** to `fixed` `[L7399]`; `linkable` functions **MUST** write the function type explicitly `[L7400]`.
- Function fields are governed by field-definition rules; the sugar **does not** apply to fields `[L7402]`.
- Function shorthand is allowed and **MUST** be expanded to this sugar form before type checking `[L7403]` (XREF §2.5, L446–447: `Name(Parameters): Type` ⇒ `Name: function(Parameters) Type`; omitted return type ⇒ 空类型).
- **Expansion** `[L7405–7410]`: when applicable, `Block` **MUST** expand to `= <function literal>`; the literal's parameter list **MUST** be the constant type's parameter list; its return type **MUST** be the constant type's return type; it **MUST NOT** carry annotations other than the return type.
- `this` **MUST** be written explicitly and **MUST NOT** be implicitly inserted `[L7414]`.
- Initialization: heading `有关初始化：` at `[L7417]` is **empty** (gap, OQ1). The general rule outside this range is that `fixed` must be initialized by `=` or `Block` unless it is in an interface body (XREF §5.3 L5693); the example restates "常量必须被初始化" `[L7485]`.
- Non-function types using the sugar are illegal: `fixed Bad: integer(4) { }` `[L7487–7489]`.

### 3.7 Parameters (§8.2)

Forms (verbatim grammar in §2.3), as a matrix:

| Property | `Parameter` (function-type constructor) | `LiteralParameter` (function literal) |
| --- | --- | --- |
| `#` (compile-time const) | optional `[L7499]` | optional `[L7503]` |
| Name | optional (`[ Name ":" ]`) `[L7499]`, but MUST appear except when useless `[L7531–7532]` | MUST appear `[L7503, L7530]` |
| `&` (by-reference) | optional, immediately before `Type` `[L7499, L7549]` | optional `[L7503]` |
| `Type` | MUST appear `[L7521]` | MAY be omitted; inferred from expected function type `[L7522–7524]` |
| Default value | optional `[L7499]` | forbidden `[L7503, L7525]` |

- Parameters are separated by `,` `[L7519–7520]`.
- A parameter whose **name is `this`** is a `this` parameter `[L7513]`; `this` **MUST** appear at most once per parameter list `[L7514]`.
- Function-type-constructor parameter **types MUST appear** `[L7521]`; function-literal parameter types **MAY** be omitted, in which case they **MUST** be inferred from the expected function type and the compiler **MUST** error when inference fails `[L7522–7524]`.
- Function-literal parameters **MUST NOT** carry default expressions `[L7525]`.
- Parameter types **MAY** use §2.5 shorthand in place of `Name ":" Type` `[L7526]` (XREF L428–441).
- If a parameter name is omitted, the parameter **MUST** be declared by type alone, and a default expression **MUST NOT** appear `[L7533–7534]`.
- `#` is a compile-time-constant-parameter convention, **not** part of the name `[L7535]`; when `#` is present the name **MAY** be omitted, written `#Type` `[L7536]`; `this` **MUST NOT** carry `#` `[L7537]`.
- `#` semantics `[L7539–7545]`: such a parameter is a compile-time constant parameter; its actual argument **MUST** be a compile-time constant (XREF §11.5); `#` **MUST** participate in function-type equality **and** in the overload signature; functions **MUST** be monomorphized per the `#` actual argument; the binding is compile-time only and **MUST NOT** be accessed at runtime.
- `&` semantics `[L7547–7551]`: `&` immediately before a type marks a **reference parameter** (XREF §3.5); reference parameters **MUST** be passed by reference; `&` is **not** part of the type and does **not** participate in type equality or in the overload signature.
- Defaults: a parameter with a default **MUST NOT** be followed by a parameter without one `[L7555]`; callers **MAY** omit defaulted parameters `[L7556]` (see §3.8).
- The grammar defines **no** `ref` / `out` / `in` keywords, **no** `params`/variadic form, and **no** `...` form. `wildcard` appears only in §2.5's shorthand (XREF L407) and in §3.6.3, not in `Parameters`.
- Valid forms from the example `[L7560–7575]`: `local Callback(integer(4));`, `local Named(value: integer(4));`, `local Ref(&integer(4));`, `local NamedRef(value: &integer(4));`, `local CompileTime(#integer(4));`, `local NamedCompileTime(#value: integer(4));`, `local WithDefault(value: integer(4) = 0): integer(4);`, `local WithThis(this: MyClass);` — note these are `local` variables whose type is written with the §2.5 function shorthand.
- Literal type inference `[L7578–7583]`: `(value) integer(4) => value` and `(value: &integer(4)) integer(4) => value` are legal against a known function type.

### 3.8 Arguments (§8.3)

- Syntax: `Arguments = [ Argument { "," Argument } ] .`, `Argument = [ Name ":" ] Expression .` `[L7601–7603]`; arguments are comma-separated `[L7615]`.
- If an argument carries a name, **all following** arguments **MUST** carry names `[L7616]` (named arguments are trailing).
- **Substitution** `[L7618–7624]`:
  - unnamed arguments are substituted **in list order** `[L7620]`;
  - named arguments are substituted into the corresponding parameter `[L7621]`;
  - an argument's value type **MUST** equal the corresponding parameter's type **or** be implicitly convertible to it `[L7622]` (XREF §4.3);
  - if the corresponding parameter carries `#`, the argument expression **MUST** be a compile-time constant `[L7623]` (XREF §11.5);
  - if multiple arguments are substituted into the same parameter, the compiler **MUST** error `[L7624]`.
- **Omission** `[L7626–7632]`: an argument **MAY** be omitted **iff all** of: the callee is a **function identifier**, and the corresponding parameter of the function that identifier denotes has a default `[L7628–7630]`. An omitted argument's value **MUST** be taken as the parameter's default `[L7631]`; a present argument's value **MUST** be the expression's return value `[L7632]`.
- Example `[L7636–7661]`: `Echo(1)`, `Echo(value: 2)`, `WithDefault()`; `Echo(value: 1, 2)` is illegal `[L7658–7660]`.

### 3.9 Methods (§8.4)

- **Definition** — a method is a function constant declared in an extension block whose `this` parameter's type is the extension target type `[L7665, L7669]`; the **target type** is the type of `this` `[L7670]`; an **extension method** is a method declared in an extension block `[L7671]`.
- A function constant **is a method iff all** of `[L7675–7679]`: it is declared in an extension block; its constant type's parameter list contains a `this` parameter; it is **not** `static`; the `this` parameter's type is the extension target type.
- Outside an extension block a function constant **MUST NOT** carry a `this` parameter `[L7680]`.
- Inside an extension block, a function constant whose `this` parameter has a **different** type **MUST NOT** be judged a method `[L7681]` (but no error is stated for it — OQ8).
- **`this` parameter rules** `[L7683–7692]`: must be explicit `[L7685]`; **MAY** appear at any position in the parameter list `[L7686]`; for a **value-type** target the type **MUST** be written with `&` `[L7687]`; for a **reference-type** target `&` **MUST NOT** be written `[L7688]`; `this` **MUST** be allowed to access private members of the target type `[L7689]`; methods **MUST** be called through a member access expression `[L7690]`; methods **MUST NOT** be called as ordinary functions with an explicit `this` argument `[L7691]`; methods **MUST NOT** enter the target type's memory layout `[L7692]`.
- **Method parameter list** = the parameter list with `this` removed `[L7696]`.
- Methods **MAY** carry generic parameters `[L7700]`.
- Example `[L7704–7785]`: `Counter.Construct/Increment` with `this: Counter`; `ValueCounter.Increment` with `this: &ValueCounter`; an external `extension for Counter { Reset }`; calls `c.Increment()`, `c.Reset()`. Illegal (commented): value-type `this` without `&` `[L7758–7766]`, `static` + `this` `[L7768–7776]`, `this` outside an extension `[L7778–7780]`, `Reset(c)` `[L7782–7784]`.

### 3.10 Overload resolution (§8.5)

- **Definitions** `[L7789–7798]`: overload resolution selects a callee among same-name function constants **or** operator definitions; a **parameter list** is the list of parameter *count, order and types*; an **exact match** is argument type == parameter type. Resolution **MUST** be done at compile time `[L7796]`, **MUST** use *only* argument types `[L7797]`, and the **return type MUST NOT** participate `[L7798]`.
- **Algorithm as specified**:
  1. Candidate set = same-name function constants (or operator definitions) `[L7793]`; generic function constants **MUST** be monomorphized **before** resolution `[L7826]`.
  2. **Exact phase** `[L7802–7803]`: if the argument types exactly match exactly one candidate ⇒ call it; if they exactly match **multiple** candidates ⇒ compiler **MUST** error.
  3. **Convertible phase** (only when there is no exact match) `[L7804–7808]`: if exactly one candidate's parameter types are the same as, or accept an implicit conversion of, the argument types ⇒ call it; if multiple accept ⇒ error, and the caller **MUST** use an explicit `Cast` so that exactly one candidate matches exactly `[L7807]`; if none accept ⇒ error.
- **Signature exclusions** `[L7810–7814]`: `&` **MUST NOT** participate in resolution `[L7812]`; parameter names and default expressions **MUST NOT** participate `[L7813]`; `#` **MUST** participate in resolution *and* signature `[L7814]`.
- **Calling convention** `[L7816–7820]`: function constants that differ **only** in calling convention **MUST NOT** coexist `[L7818]`; the calling convention **MUST NOT** participate in resolution `[L7819]` and **MUST NOT** participate in the signature `[L7820]` (XREF §3.6.3).
- **Applicability** `[L7822–7826]`: operator definitions **MUST** dispatch by this chapter's rules `[L7824]`; method resolution **MUST** proceed in the declared parameter order and `this` **MUST NOT** be moved to another position `[L7825]`; generic functions are monomorphized first `[L7826]`.
- No betterness/ranking order exists: any ambiguity is an **error**, resolved only by an explicit `Cast` `[L7806–7807]`.
- Example `[L7830–7870]`: `Calculate(integer(4), integer(4))` vs `Calculate(float(4), float(4))`; `Calculate(1, 2)` → integer version; `Calculate(1.0, 2.0)` → float version; `Calculate(true, false)` illegal; return-type-only overload illegal `[L7860–7863]`; calling-convention-only overload illegal `[L7865–7868]`.

### 3.11 Modifier framework (§9)

- **Definition** — a modifier is the syntactic form placed before an identifier kind that qualifies the identifier's behavior `[L7874]`.
- Categories `[L7877–7895, L7919–7923]`:
  - **Accessibility**: `public`, `private`, `protected`, `scoped` `[L7919]`;
  - **Behavior-limiting**: `readonly` `[L7883, L7920]`;
  - **Behavior-enabling**: `implicitable` `[L7885, L7921]`;
  - **Optimization**: `verbatim` `[L7887, L7922]`;
  - **Other**: `manual`, `abstract`, `sealed`, `virtual`, `override`, `aligned(N)`, `static` `[L7889–7895, L7923]`.
- Placement/combination `[L7925–7931]`: modifiers **MUST** precede their target `[L7927]`; multiple modifiers **MAY** appear `[L7928]`; **any order** is allowed `[L7929]`; modifiers **MUST** be separated by whitespace `[L7930]`; the **same modifier MUST NOT repeat** `[L7931]` (but see `scoped` at L8246 → OQ11).
- A modifier **MUST** only modify targets it allows `[L7935]`; per-modifier lists are in §9.1–§9.15; semantics mapping is §2.9 (XREF).
- **Modifier × target matrix** (from the §9.x pages):

| Modifier | Legal targets (in-range) | Hard constraints |
| --- | --- | --- |
| `public` | any identifier (accessibility) `[L8052–8063]` | ≤ 1 base accessibility modifier `[L7996]`; no differentiation in global scope `[L8071]` |
| `private` | any identifier (accessibility) `[L8099–8110]` | ≤ 1 base modifier `[L7996]`; extension-declared `private` visible only in that extension block `[L8110]` |
| `protected` | any identifier (accessibility) `[L8159–8171]` | ≤ 1 base modifier `[L7996]`; no differentiation in global **or** module scope `[L8179]` |
| `scoped` | any identifier (trim) `[L8221–8228]` | MUST NOT widen base `[L7995, L8247]`; targets named only `[L8248]`; union within one, intersection across `[L8245–8246]` |
| `readonly` | fields in `class` / `structure` `[L8382–8383]` | MUST NOT co-occur with `fixed` `[L8384]`; only initialized or assigned in a constructor `[L8385]` |
| `implicitable` | conversion operators `[L8399–8400]` | only conversion operators; lossy implicit conversion allowed `[L8406]` |
| `verbatim` | code block, variable (`local`), constant (`fixed`) `[L8475–8478]` | MUST directly precede the block `[L8483]`; no optimization `[L8485]`, no compile-time evaluation `[L8487]` |
| `manual` | `heap` definitions `[L8522]` | MUST precede `heap` `[L8523]`; no NOLAA auto-release `[L8524]`; `dispose` required `[L8525]` |
| `abstract` | `class` declarations `[L8539]` | not instantiable `[L8540]`; may carry inheritance list `[L8541]`; ctor/dtor inherit `[L8542–8543]` |
| `sealed` | `class` declarations; override methods `[L8557–8559]` | class not inheritable; override not further overridable |
| `virtual` | methods `[L8573]` | MUST NOT modify `structure` members `[L8574]`; MUST provide a body `[L8575]` |
| `override` | methods meeting class override requirements `[L8589]` | MUST NOT modify `structure` members `[L8590]`; covariant `this` target allowed `[L8591]` |
| `aligned(N)` | `structure` declarations; `structure` fields; union members `[L8621–8624]` | N ≥ 0 `[L8607]`; union member = byte offset `[L8618–8620]`; software emulation if unsupported `[L8614, L8617]` |
| `static` | `class` declarations; `class`/`structure` fields; constants in `class`/`structure` extension blocks **without** `this`; accessors in those extension blocks `[L8663–8667]` | forbidden: `this`-carrying constants, `local`, interface members, declarations of `structure`/`interface`/`union`/`variant`/`enumeration`/`module` `[L8668–8672]` |

- Example `[L7941–7958]`: `public sealed class Singleton`; `public static readonly Instance: Singleton;`; illegal `public public class Bad` `[L7951–7953]`; illegal modifier after target `class public Bad` `[L7955–7957]`.

### 3.12 Accessibility modifiers in general (§9.1)

- Grammar `[L7965–7969]`: `AccessibilityModifier = BaseAccessibilityModifier | ScopedModifier`; base = `public` | `private` | `protected`.
- Two dimensions (spec table `[L7985–7988]`): **base accessibility** (`public`/`private`/`protected`) gives the base set of accessible objects; the **trimming** modifier (`scoped`) narrows that set by named targets.
- **Final accessibility = base accessibility ∩ *all* trims** `[L7992]`; when the base modifier is omitted the base accessibility **MUST** be `private`, unless the kind provides otherwise `[L7993]`; enum members, variant states and union members **MUST** have base accessibility `public` `[L7994]`; trims **MUST NOT** widen base accessibility `[L7995]`; at most one base modifier `[L7996]`.
- Scope behavior `[L7998–8003]`: accessibility modifiers **MAY** appear in any scope `[L8000]`; in the **global** scope base modifiers **MUST NOT** produce differentiation `[L8001]`; in **module** scope `protected` **MUST NOT** differentiate `[L8002]`; non-differentiation **MUST NOT** be an error `[L8003]`; NOTE explains why `private` degenerates to "all" and `protected` degenerates to nothing `[L8014–8018]`.
- **Function values** `[L8005–8012]`: define **access right** (访问权) as the set of members a function value may access; a function **constant**'s access right is that of its declaration site `[L8008]`; a function **literal**'s access right is that of its writing site `[L8009]`; a function value **MUST** carry its access right `[L8010]`; if the accessible range of the **location carrying** the function value is **larger** than the function value's access right, the compiler **MUST** error `[L8011]`; range size is decided by scope containment `[L8012]`.
- Example `[L8022–8048]`: `private fixed Hidden()`; assigning `Hidden` to a `public` function-typed field is illegal `[L8030–8032]`; `public scoped(Animal) fixed Shared()` is legal `[L8041–8045]`.

### 3.13 `public` (§9.2)

- Keyword `public` `[L8061]`; a `public` identifier's accessible objects are **all scopes** `[L8062]`; it **MUST** be accessible from any scope `[L8063]`.
- The modifier list **MUST** precede the identifier kind `[L8067]`; in global scope `public` **MUST NOT** differentiate `[L8071]`.
- Example `[L8075–8095]`: `Animal.Name` public, read from derived `Cat.Read(this: Cat)`.

### 3.14 `private` (§9.3)

- Keyword `private` `[L8108]`.
- Accessible objects: the **declaring scope**, the **members of the type it belongs to**, and **functions carrying a `this` parameter of that type** `[L8109]`.
- **Narrowing exception**: `private` on an **extension member** **MUST** be visible **only inside that extension block** `[L8110]`.
- Default accessibility is governed by §9.1 `[L8111]`.
- Modifier list before the identifier kind `[L8115]`.
- In a type declaration body the accessible object is that type's **type scope** `[L8119]`, and **all other members of that type MUST** be able to access the identifier `[L8120]`.
- In global scope `private` **MUST NOT** differentiate `[L8121]`.
- Example `[L8125–8155]`: default-private field `Count` accessed by the same type's method `[L8134–8138]`; another type's `Peek(target: Counter)` cannot access `target.Count` `[L8146–8151]`.

### 3.15 `protected` (§9.4)

- Keyword `protected` `[L8168]`.
- Accessible objects: the **declaring scope** and its **derived scopes** `[L8169]`; a **derived scope** is every type that directly or indirectly inherits from the declaring type `[L8170]` (XREF §3.7.1, §3.7.7); both classes of scope **MUST** be able to access it `[L8171]`.
- In **global or module** scope `protected` **MUST NOT** differentiate `[L8179]`.
- Example `[L8183–8217]`: `Animal.Name` protected; `Cat.Read` legal `[L8196–8200]`; `Stranger.Peek(target: Animal)` illegal `[L8209–8213]`.

### 3.16 `scoped` (§9.5)

- Grammar `[L8224–8228]`: `scoped ( ScopedTarget { "," ScopedTarget } )`; `ScopedTarget = [ "sealed" ] Identifier | "operator" [ Identifier | OperatorSymbol ]`.
- `sealed` in a target is the **inheritance-item cancellation marker**, and is a keyword `[L8234–8235]`; `operator` is the **operator-scope marker**, and is a keyword `[L8236–8237]`.
- Terms `[L8241–8248]`: the **trim range** is the set of accessible objects built from all targets `[L8243]`; final accessibility = base ∩ all trims `[L8244]`; **multiple targets within one `scoped` MUST union** `[L8245]`; **multiple `scoped` MUST intersect** `[L8246]`; `scoped` **MUST NOT** widen base accessibility `[L8247]`; targets **MUST** be named targets, **MUST NOT** be scope forms `[L8248]`.
- Target semantics `[L8250–L8261]`:
  - **self items** of `X` = `X` itself and its members `[L8252]`;
  - **inheritance items** of `X` = all objects directly or indirectly inheriting from `X`, and their members `[L8253]`; when `X` is a **member**, its inheritance items are `X`'s **overrides** `[L8253–8254]`;
  - plain `Identifier` target ⇒ trim range = self items ∪ inheritance items `[L8255]`;
  - `sealed` on a target ⇒ inheritance items **cancelled**, trim range = self items only `[L8256]`;
  - a type `X`'s **operators** = operators whose parameter list contains `X` `[L8257]`; an `operator` target **MUST** appear in a type declaration body **or** an extension block `[L8258]`; `operator` alone ⇒ the declaring type's operators `[L8259]`; `operator OperatorSymbol` ⇒ that symbol's operators of the declaring type `[L8260]`; `operator Identifier` ⇒ conversion operators whose conversion target type is that `Identifier` `[L8261]`.
- Resolution `[L8263–8265]`: a relative identifier target **MUST** be resolved relative to the `scoped` site's scope.
- Example `[L8269–8368]`: `Animal` declares `protected scoped(Animal) VisibleToAnimal`, `protected scoped(Speak) VisibleToSpeak`, `protected scoped(sealed Speak) VisibleToSpeakOnly`, plus `public virtual fixed Speak(this: Animal)`; `Cat`'s `override Speak` sees `VisibleToAnimal` and `VisibleToSpeak` but **not** `VisibleToSpeakOnly` `[L8300–8313]`. `structure A` has `public scoped(operator, A) Member: integer(4)`; `operator +(a: A, b: A)` and `operator +(a: B, b: A)` both access `A.Member` `[L8315–8341]`; `structure B` cannot access `A.Member` from its own `operator ==` `[L8356–8362]`.

### 3.17 `readonly` (§9.6)

- Keyword `readonly` `[L8381]`; **only** fields in `class` and `structure` `[L8382–8383]` (XREF §3.8.1: field definitions have no `fixed` kind keyword — see OQ19).
- **MUST NOT** co-occur with `fixed` `[L8384]`.
- Only `readonly` members may be initialized, **or** assigned in a constructor `[L8385]` (wording ambiguous — OQ19).
- No example block in this subsection.

### 3.18 `implicitable` (§9.7)

- Keyword `implicitable` `[L8398]`; **only** conversion operators `[L8399–8400]` (XREF §4.9).
- With `implicitable`, the conversion **MAY** be invoked implicitly `[L8404]`; without it, the conversion **MUST** be invoked only through a type-conversion expression `[L8405]`.
- The implicit invocation **MAY** be lossy, i.e. it is **not** constrained by the losslessness rule for implicit conversions `[L8406]` (XREF §1.1).
- Example `[L8410–8461]`: `implicitable operator HSLColor: function(from: RGBColor) HSLColor` `[L8418]`; non-implicitable conversion back to `RGBColor` `[L8432]`; `(rgb as HSLColor)` legal `[L8446]`; `local b: HSLColor = rgb;` legal `[L8449]`; `local bad: RGBColor = hsl;` illegal `[L8452–8453]`; `implicitable operator +` illegal `[L8459–8460]`.

### 3.19 `verbatim` (§9.8)

- Keyword `verbatim` `[L8474]`; targets: **code block, variable (`local`), constant (`fixed`)** `[L8475–8478]`.
- A `verbatim` **block** is a code block carrying `verbatim` `[L8482]`; `verbatim` **MUST** be placed **directly before** the code block `[L8483]`; the block's content **MUST** conform to the syntax of the enclosing scope `[L8484]`.
- The compiler **MUST NOT** perform **any** compile-time optimization on the target `[L8485]`; **necessary code generation** is allowed but **MUST NOT** change source structure `[L8486]`; the compiler **MUST NOT** perform compile-time evaluation inside the target `[L8487]` (XREF §11.1).
- NOTE `[L8489–8491]`: instruction selection, register allocation, and ABI stack/parameter layout are *not* compile-time optimization and are therefore unaffected.
- Example `[L8495–8508]`: `verbatim { local sum: integer(4) = 1 + 2; }` — the initializer must not be folded.

### 3.20 `manual` (§9.9)

- Keyword `manual` `[L8521]`; **only** `heap` definitions `[L8522]` (XREF §5.4); **MUST** precede `heap` `[L8523]`.
- A `manual` heap allocation unit **MUST NOT** be released automatically by NOLAA `[L8524]`; the developer **MUST** release it with `dispose` `[L8525]` (XREF §5.8).

### 3.21 `abstract` (§9.10)

- Keyword `abstract` `[L8538]`; **only** `class` declarations `[L8539]`.
- An abstract class **MUST NOT** be instantiated `[L8540]`; it **MAY** carry an inheritance list `[L8541]`.
- An abstract class's `Construct` / `Destruct` **MAY** be inherited `[L8542]`; when a **non-abstract** class inherits them, they **MUST** be recognized as that class's constructor/destructor `[L8543]`.
- The range defines **no** `abstract` method form (only `class`); see OQ21.

### 3.22 `sealed` (§9.11)

- Keyword `sealed` `[L8556]`; targets: `class` declarations and **override methods** `[L8557–8559]`.
- Semantics implied by the definition `[L8547]`: a sealed class cannot be inherited; a sealed override cannot be overridden further.

### 3.23 `virtual` (§9.12)

- Keyword `virtual` `[L8572]`; **only** methods `[L8573]`; **MUST NOT** modify `structure` members `[L8574]`; a `virtual` method **MUST** provide a body `[L8575]` (XREF §8.1).

### 3.24 `override` (§9.13)

- Keyword `override` `[L8588]`; **only** methods that satisfy the class method-override requirements `[L8589]` (XREF §3.7.1, §8.4); **MUST NOT** modify `structure` members `[L8590]`.
- The `this` parameter's target type **MAY** be covariantly replaced `[L8591]`.

### 3.25 `aligned(N)` (§9.14)

- Grammar `[L8598]`: `aligned ( IntegerLiteral )`; N is the literal's value **in bytes** `[L8606]`; N **MUST** be a non-negative integer `[L8607]`.
- On a **`structure` declaration** `[L8609–8614]`: N **MUST** be the default alignment requirement of all fields `[L8610]`; default field offsets **MUST** be computed with that requirement `[L8611]`; a field-level `aligned(M)` **MUST** align that field to M bytes and **overrides** the structure default `[L8612]`; **N = 0** ⇒ all fields default to 0-byte alignment, cancelling only padding ⇒ **compact layout** `[L8613]`; if the target platform's alignment capability cannot satisfy N, the compiler **MUST** emulate it in software `[L8614]`.
- On a **`structure` field** `[L8615–8617]`: N **MUST** be that field's alignment requirement; software emulation applies again `[L8617]`.
- On a **union member** `[L8618–8620]`: N **MUST** be the member's **offset** relative to the union's base start (XREF §3.7.6); that offset **MUST NOT** be adjusted by the platform's alignment capability `[L8620]`.
- Allowed targets: `structure` declarations, `structure` fields, union members `[L8621–8624]`.
- Example `[L8628–8649]`: `aligned(0) structure Compact { public A: integer(1); public B: integer(4); }`; `aligned(8) structure Aligned`; `aligned(8) structure Mixed { public A: integer(1); public aligned(1) B: integer(1); }` (field overrides structure default).

### 3.26 `static` (§9.15)

- Keyword `static` `[L8662]`; **allowed targets**: `class` declarations `[L8664]`; fields in `class` and `structure` `[L8665]`; constants in `class`/`structure` extension blocks **that contain no `this` parameter** `[L8666]`; accessors in `class`/`structure` extension blocks `[L8667]`.
- **Forbidden targets** `[L8668–8672]`: constants carrying `this` `[L8669]`; variables (`local`) `[L8670]`; interface members `[L8671]`; declarations of `structure`, `interface`, `union`, `variant`, `enumeration`, `module` `[L8672]`.
- **Members** `[L8674–8679]`: `static` members **MUST NOT** enter the instance memory layout `[L8676]`; they **MUST** be accessed via the **type identifier** through a member access expression and **MUST NOT** be accessed through an instance `[L8677]`; when `static` modifies a **member of a generic type**, the member **MUST** attach to the **monomorphized** type `[L8678]`; when a static field's initial value is compile-time determinable it **MUST** be emitted into a **writable section**, otherwise the **Launcher** initializes it at runtime `[L8679]` (XREF §14.2).
- **Static constructor** `[L8681–8684]`: defined as a constant **named `Construct`** with **void return type** and `static` `[L8683]`; it **MUST** execute before `Main` `[L8684]` (XREF §14.1).
- **Static class** `[L8686–8690]`: its members **MUST** all be `static` `[L8688]`; it **MUST** be a global singleton `[L8689]`; creating an instance is **forbidden** `[L8690]`.
- **Type-level accessors** `[L8692–8695]`: their members **MUST** be constants without a `this` parameter `[L8694]`; the accessor's **Backing MUST** be a `static` field of the same type, a global variable, or a global constant `[L8695]` (XREF §3.8.2, §3.8.1).
- NOTE `[L8697–8700]`: modules are for identifier organization and inline initialization and provide **no** initialization hook; static classes can declare a static constructor to carry arbitrary initialization logic.
- Example `[L8704–8780]`: `Vector2D` with `public static fixed GetX(v: Vector2D)` `[L8714]` and `public static InstanceCount: integer(4) = 0;` `[L8721]`; `Container<T>` with `public static Count` `[L8726–8729]`; `static class CounterSingleton` with `public static fixed Construct()` `[L8733–8744]`; access `Vector2D.GetX(v)`, `Container<integer(4)>.Count = 5`, `CounterSingleton.Count` `[L8748–8756]`. Illegal (commented): `static structure Bad` `[L8759–8761]`, `static` + `this` `[L8763–8771]`, `CounterSingleton()` `[L8773–8775]`, `v.GetX(v)` `[L8777–8779]`.

### 3.27 Generic parameters (§10.1)

- Grammar `[L8789–8796]`: `GenericParameters = "<" GenericParameterDefinition { ";" GenericParameterDefinition } ">"`; `GenericParameterDefinition = GenericParameter [ ":" Constraint { "," Constraint } ]`; `Constraint = TypeKind | Identifier`; `TypeKind = "class" | "structure" | "interface" | "enumeration" | "variant" | "union" | "basic"`.
- **Separators** `[L8812–8813]`: generic-parameter definitions are separated by `;` (not `,`); constraints are separated by `,`.
- Constraint aggregation **MUST** have **AND** semantics `[L8814]`; generic parameters **MUST** appear **after the name** `[L8815]`.
- A **kind** constraint (`class`/`structure`/`interface`/`enumeration`/`variant`/`union`/`basic`) requires the passed type to belong to that kind `[L8816]`; a **concrete type** constraint requires the passed type to **inherit, implement, or equal** it `[L8817]`.
- **Scope** `[L8819–8824]`: a generic parameter's scope is the **declaration body** `[L8821]`; it **MUST** be accessible in its scope `[L8822]`; it **MUST** be a compile-time constant `[L8823]`; it **MAY** be used directly in compile-time code `[L8824]` (XREF §11.7).
- **Allowed positions** `[L8828–8837]`: after the name of `fixed`, `linkable`, `class`, `structure`, `interface`, `union`, `variant`, compile-time function names, `extension`.
- **Forbidden** `[L8838–8844]`: `enumeration` **MUST NOT** carry generic parameters `[L8838]`; field definitions **MUST NOT** `[L8839]`; `local` **MUST NOT** `[L8840]`; `heap` **MUST NOT** `[L8841]`; `operator` **MUST NOT** `[L8842]`; `accessor` **MUST NOT declare** generic parameters but **MAY borrow** those of its enclosing declaration `[L8843–8844]`.
- **Monomorphization** `[L8846–8851]`: **MUST** run at compile time `[L8848]`; the **unit** is the declaration carrying generic parameters `[L8849]`; it **MUST** be initiated through a monomorphization expression `[L8850]`; failure ⇒ compiler **MUST** error `[L8851]`.
- Example `[L8855–8914]`: `structure Box<T: structure>`, `structure Pair<T: class; U>`, `structure SortedList<T: class, IComparable>` (multiple constraints), `structure BasicBox<T: basic>`, `fixed Echo<T>(value: T): T`, `Echo<integer(4)>(42)`; illegal field generic `[L8900–8905]`; constraint violation `Bad<integer(4)>` `[L8907–8913]`.

### 3.28 Monomorphization expressions (§10.2)

- Grammar `[L8921]`: `( Identifier | Expression ) "<" [ Type ] { ";" [ Type ] } ">"`.
- Generic actual arguments **MUST** be separated by `;` `[L8930]`; a generic actual argument **MAY** be left **empty**, and empty means the **empty type** `[L8931]` (XREF §3.6).
- The monomorphized identifier **MUST** carry generic parameters `[L8935]`; the **number of generic actual arguments MUST equal** the number of generic parameters `[L8936]`; each actual argument **MUST** satisfy the corresponding constraint `[L8937]`.
- Monomorphization **MUST** run at compile time `[L8941]`; failure ⇒ compiler **MUST** error `[L8942]`.
- The expression's value is the monomorphized result of the identifier `[L8946]`; the **kind** of the result **MUST** be the same as the kind of the monomorphized identifier `[L8947]`.
- Volume optimization of monomorphized instances is delegated to §14.3.2 (XREF, anchor not located) `[L8949]`.
- No example block in §10.2. See `Container<integer(4)>.Count` at `[L8753]` for a member-access use, and `Echo<integer(4)>(42)` at `[L8898]` for a call use.

## 4. Exact keyword spellings (as written in the range)

| Spelling | Role in this range | Lines |
| --- | --- | --- |
| `module` | module declaration identifier kind | L7334 (kind list XREF L517) |
| `fixed` | constant kind; function-definition sugar only applies to it | L7384, L7399 |
| `function` | function type constructor (`FunctionTypeConstructor`) | L7384 |
| `this` | distinguished parameter name; keyword (XREF L529) | L7513, L7514, L7537, L7685 |
| `owner` | accesses shadowed names in the directly enclosing scope; keyword (XREF L529) | L7294–7298 |
| `public` | accessibility modifier; keyword | L8055, L8061 |
| `private` | accessibility modifier; keyword | L8102, L8108 |
| `protected` | accessibility modifier; keyword | L8162, L8168 |
| `scoped` | accessibility trimming modifier; keyword (XREF L521) | L8224 |
| `sealed` | (a) modifier (class / override method); (b) **inheritance-item cancellation marker inside `scoped`** | L8234–8235, L8226, L8550, L8556 |
| `operator` | (a) operator-scope marker inside `scoped`; (b) identifier kind (XREF L517) | L8236–8237, L8227 |
| `readonly` | behavior-limiting modifier; keyword | L8375, L8381 |
| `implicitable` | behavior-enabling modifier; keyword | L8392, L8398 |
| `verbatim` | optimization modifier; keyword | L8468, L8474 |
| `manual` | modifier for `heap`; keyword | L8515, L8521 |
| `abstract` | class modifier; keyword | L8532, L8538 |
| `virtual` | method modifier; keyword | L8566, L8572 |
| `override` | method modifier; keyword | L8582, L8588 |
| `aligned` | parameterized alignment modifier (`aligned(N)`); keyword | L8598, L8604 |
| `static` | static-member modifier; keyword | L8656, L8662 |
| `class` | TypeKind constraint keyword / class kind | L8796 |
| `structure` | TypeKind constraint keyword | L8796 |
| `interface` | TypeKind constraint keyword | L8796 |
| `enumeration` | TypeKind constraint keyword | L8796 |
| `variant` | TypeKind constraint keyword | L8796 |
| `union` | TypeKind constraint keyword | L8796 |
| `basic` | TypeKind constraint keyword (basic types) | L8796, L8808 |
| `linkable` | kind that may carry generics but must spell its function type | L7400, L8830 |
| `extension` | extension block; may carry generics | L7401, L8837 |
| `local` | variable kind; cannot carry generics, cannot be `static` | L8840, L8670 |
| `heap` | heap kind; `manual` target; cannot carry generics | L8522, L8841 |
| `use` | import statement (XREF §5.6; not in range) | L8982, L9011 (out of range) |
| `Cast` | explicit type-conversion expression used to disambiguate overloads | L7807, L8446 |
| `dispose` | manual release | L8525 |
| `#` | compile-time marker: `#`-parameter convention, actual argument must be compile-time constant | L7535–7537, L7541–7545, L7623, L7814 |
| `&` | reference-parameter marker; not part of the type | L7499, L7503, L7549–7551, L7687–7688, L7812 |
| `;` | generic-parameter/generic-actual separator; also statement terminator (XREF §2.8) | L8789, L8921 |
| `<` `>` | generic parameter/actual delimiters | L8789, L8921 |
| `,` | parameter/argument/constraint/`scoped`-target separator | L7497, L7601, L8224, L8791 |

Spelling notes: all modifiers are **lowercase**; `Cast` is capitalized; `aligned` takes a **parenthesized `IntegerLiteral`** with no space documented; `#`/`&` are punctuation, not keywords (`#` is explicitly "not part of the parameter name" L7535); `sealed` has **two distinct roles** (modifier vs in-`scoped` marker) and the spec gives no disambiguation rule beyond grammar position (OQ12).

## 5. Example inventory (all `novasharp` blocks in range)

| Lines | Section | One-line description |
| --- | --- | --- |
| 7302–7327 | §6.4 | Shadowing ladder: global `a`, `Main`'s `a`, block `a`, `owner.a`, `owner.owner.a` |
| 7359–7375 | §7 | Nested block-level modules; commented-out illegal "module as type" |
| 7421–7490 | §8.1 | Function sugar forms (explicit type + `Block`, shorthand, desugared literal, generic `Echo<T>`, extension methods with explicit `this`, function field) + commented illegal forms |
| 7560–7594 | §8.2 | Every parameter form: unnamed, named, `&`, `#`, default, `this`; function-literal type inference; commented illegal literal without name / with default |
| 7636–7661 | §8.3 | Positional, named, and default-omitted calls; commented illegal positional-after-named |
| 7704–7785 | §8.4 | `Counter` methods, value-type `this: &ValueCounter`, external `extension for Counter`, receiver calls; commented illegal variants |
| 7830–7870 | §8.5 | Two `Calculate` overloads and dispatch by argument type; commented illegal no-match, return-only overlap, calling-convention-only overlap |
| 7941–7958 | §9 | `public sealed class`, `public static readonly` field; commented illegal duplicate `public`, modifier after target |
| 8022–8048 | §9.1 | `private Hidden` escaping into `public Exposed` (illegal); `public scoped(Animal)` combination (legal) |
| 8075–8095 | §9.2 | `public` field read through a derived class method |
| 8125–8155 | §9.3 | Default-`private` field accessed in-type (legal) vs from another type (illegal) |
| 8183–8217 | §9.4 | `protected` field visible in derived type (legal), invisible to unrelated type (illegal) |
| 8269–8368 | §9.5 | `scoped` self/member/`sealed` targets across `Animal`/`Cat` overrides; `scoped(operator, X)` fields and operator access in `A`/`B` |
| 8410–8461 | §9.7 | `implicitable` conversion (legal implicit) vs non-implicitable (cast required); commented illegal implicit `RGBColor` and `implicitable operator +` |
| 8495–8508 | §9.8 | `verbatim` block preventing folding of `1 + 2` |
| 8628–8649 | §9.14 | `aligned(0)` compact struct, `aligned(8)` default, field-level `aligned(1)` override |
| 8704–8780 | §9.15 | Static method/field, generic-type static field, static class with static ctor, access via type; commented illegal static struct / static `this` / static-class instance / instance access |
| 8855–8914 | §10.1 | Generic structs with kind/concrete/multi constraints, generic function + explicit instantiation; commented illegal field generics and constraint violation |

## 6. Open questions (ambiguities / gaps / inconsistencies)

1. **Empty §8.1 initialization heading — L7417.** `有关初始化：` has no content. *Proposal:* the `Block` is the initializer; `fixed` must still be initialized per XREF §5.3 L5693 and evaluated per XREF §14.2; no sugar-specific rule.
2. **No import/qualification mechanics in §7 — L7329–7375.** §7 defines module scope and nesting but no import production, no module-member access operator, no namespace-merging rules; `use` lives in XREF §5.6 (L5943) and is only mentioned in §11.2 (L8982, L9011). *Proposal:* module = lexical scope; `use` imports members into the current scope for subsequent code only (XREF L5959–5960); qualified access via `.` in the member-access form used for `owner.` (L7295) and static members (L8677). Note the tension: L5960 forbids hoisting while L7277 says definition order does not matter (different rules — `use` is not a definition).
3. **Is uniqueness per kind-namespace or one namespace per scope? — L7264.** The rule is unqualified ("同名标识符") and the only exceptions are L7268–7273, so a type name, a module name, a value name, and a function constant with the same name in the same scope would all collide. *Proposal:* one namespace per scope; implement exactly as written; kinds get no separate namespace.
4. **Default arguments and overload selection — L7628–7630 vs L7802–7808.** The spec says resolution is by argument types only and that omission requires "the function identifier", but gives no candidate-filtering rule when arity differs. *Proposal:* keep candidates whose non-defaulted prefix is compatible with the supplied arguments, apply exact/convertible phases to the supplied arguments, error on multiple survivors, then fill defaults; "function identifier" means a name denoting a `fixed` (not an arbitrary function-typed expression), consistent with L7621.
5. **Named arguments and overload selection — L7616, L7621 vs L7797, L7813.** Parameter names must not participate in resolution, yet named arguments require mapping names to parameters. *Proposal:* select candidates by types only; then bind names; a name that does not exist in the selected signature, or binding the same parameter twice, is an error (L7624).
6. **Whether `#` affects "exact match" — L7795 vs L7814.** "Exact match" is defined purely on argument/parameter *types*, but `#` must participate in the signature. *Proposal:* `#`-ness is part of candidate identity, not of exactness; a `#` parameter with a non-constant argument is rejected by L7623 before/independently of matching.
7. **`this` position vs call-site binding — L7686 vs L7690, L7825.** `this` may occupy any position, methods must be called through member access, and `this` must not be "moved". How positional call arguments map onto parameters when `this` is not first is unstated. *Proposal:* the receiver supplies `this` in its declared slot; remaining arguments bind left-to-right (or by name) to the non-`this` parameters; call-site arity excludes `this`; method parameter list excludes `this` (L7696).
8. **`this` parameter of the wrong type inside an extension block — L7681.** Such a constant "must not be judged a method", but no prohibition is stated (L7680 only covers *outside* extension blocks). *Proposal:* make it an error; otherwise it is an uncallable function constant carrying `this` whose receiver semantics are undefined.
9. **`private` declared in an extension block — L8110 vs L8109, L8120, L7689.** L8110 restricts visibility to that extension block, while L8109/L8120 grant access to the owning type's members and L7689 requires `this` to reach the target's privates. *Proposal:* L8110 is a narrowing exception for extension-declared members; type-body-declared privates follow L8109/L8120.
10. **Default accessibility of declarations themselves — L7993, L8001.** Types, modules, `linkable`s and top-level `fixed`s have no modifier by default; L7993 says base accessibility defaults to `private`, but in global scope this is a no-op (L8001). Also no rule bars a `private` type from appearing in a wider-accessible signature (only *function values* are constrained, L8011). *Proposal:* default base = `private`; nested/inner types used in a wider signature are an error by analogy with L8011 (flag as an extension).
11. **Repeated `scoped` vs "same modifier MUST NOT repeat" — L7931 vs L8246.** L8246 explicitly requires *multiple* `scoped` occurrences to intersect, contradicting L7931. *Proposal:* `scoped` is the documented exception (each occurrence is a distinct trim); all other modifiers may appear at most once; `aligned(4) aligned(8)` is an error.
12. **`scoped` target kinds and grammar — L8226–8228, L8248, L8265.** `Identifier` may denote a type, a member, a module, a generic parameter, etc.; `sealed` is grammar-restricted to `Identifier` targets; `operator` targets are only legal in a type body/extension (L8258). No rule limits which named kinds are valid targets. *Proposal:* resolve like any name at the `scoped` site (L8265); allow types and members; `sealed` only with `Identifier`; `operator` outside a type body/extension is an error.
13. **What counts as an "override" for member targets — L8253–8254.** *Proposal:* only methods carrying `override` (XREF L8588–8589) count; same-signature non-override methods are distinct members and are not included.
14. **`aligned(0)` and non-power-of-two N — L8607, L8613.** "0-byte alignment" is not a valid alignment; N = 3, 6, … is unspecified. *Proposal:* N = 0 ⇒ alignment 1 (packed, no padding); non-power-of-two N is either rejected or implemented via explicit padding + `align 1` global, with L8614/L8617 software emulation covering unsupported values; document the choice.
15. **Union member modifier grammar — L8618–8624.** §9.14 allows `aligned(N)` on union members, but union-member syntax is XREF §3.7.6, and XREF §3.8.1 L3168–3173 forbids fields in unions. *Proposal:* union member = `{ Modifier } Name ":" Type [ "=" Expression ]` with `aligned(N)` read as a base-relative byte offset (L8619).
16. **Generic constraints are under-powered — L8794.** `Constraint = TypeKind | Identifier` forbids constructed constraints (e.g. `IComparable<integer(4)>`), pointer constraints, and compound forms. Also no rule forbids duplicate generic-parameter names or collisions with outer names, and no explicit shadowing rule for generic parameters. *Proposal:* implement `Constraint` literally (bare identifier/kind); treat duplicates per L7264; generic parameters shadow outer names per L7286–7289.
17. **Parsing `MonomorphizationExpression` — L8921.** `<` conflicts with comparison; the left side may be an arbitrary `Expression`; empty actual arguments mean "empty type" (L8931); nested instantiations produce `>>`. *Proposal:* parse as a postfix only when the operand denotes a generic declaration and brackets balance; `X<>` = one empty-type argument when arity is 1, `X<;>` = two empty arguments; the lexer must split `>>` in nested type positions; otherwise arity (L8936) or constraint (L8937) errors.
18. **`verbatim` on variables/constants vs "directly before the code block" — L8475–8483.** The "directly before" rule only makes sense for blocks. *Proposal:* for `local`/`fixed`, `verbatim` applies to the initializer and to all uses of the binding (no folding, no compile-time evaluation, L8487); for blocks it covers the block body.
19. **`readonly` + `fixed` exclusion is close to vacuous — L8384 vs XREF L3102.** `FieldDefinition` has no `fixed` kind keyword, so a field can never syntactically be `fixed`. Also L8385's Chinese wording "仅允许 readonly 成员被初始化" can be read as "only readonly members may be initialized". *Proposal:* read L8385 as "readonly members may be assigned only at initialization or inside a constructor"; read L8384 as a guard against a type-body constant being `readonly` — if such a form is ever admitted, `readonly fixed` is an error.
20. **Modifiers are not in any signature — L7264, L7812–L7814.** Nothing explicitly states that accessibility/`readonly`/`verbatim`/`manual`/`scoped`/`virtual`/`override`/`sealed`/`static` are excluded from signatures, yet none may disambiguate. *Proposal:* signature = name + ordered parameter types (+ `#` marks, + `this` type for methods, − `this` for method lists); every other modifier is invisible; two declarations differing only in such a modifier are L7264 errors.
21. **Virtual/abstract/sealed combinations — L8539–L8540, L8557–L8559, L8575.** There is no `abstract` *method* form (so classes cannot force overrides except via interfaces, XREF §3.7.3); `sealed override` is implied by L8557–8559 but not spelled out; `sealed abstract class`, `virtual override`, `static abstract`, `abstract override` are unaddressed. *Proposal:* `sealed abstract` and `virtual override` (both on one member) are errors; `sealed override` is legal and terminates overriding; `static` never combines with `virtual`/`override`/`abstract`.
22. **Static constructor shape and ordering — L8683–8684.** "named `Construct`, void return, `static`" leaves parameters, visibility, multiplicity and order among several static constructors undefined; only "before `Main`" is stated. *Proposal:* at most one static constructor per type (`Construct` + no `this` + no parameters, per the example L8740), executed before `Main` in declaration/file order per XREF §14.2; any parameterized `static Construct` is an error.
23. **"Parameter name useless in context" is undefined — L7531–7532 vs L7397.** §8.1 requires function-typed constants to carry parameter names, while §8.2 lets a function-type constructor omit them when "the name is useless in the context"; no predicate is given, and only function-type declarations/examples show unnamed forms (`local Callback(integer(4))`, `local Ref(&integer(4))` at L7562–7565). *Proposal:* treat names as required whenever the callee can be invoked by name or with named arguments (i.e. for `fixed` constants per L7397) and as omissible only for pure type positions such as a parameter type, field type, or local variable type where the value is never called by name; when such a nameless type is later called with a named argument, diagnose at the call site.

## 7. LLVM 18 codegen implications

Everything below is *derived* from the in-range rules; where the spec is silent it is marked OQ.

### 7.1 Name mangling — required inputs

| Input | Include? | Source |
| --- | --- | --- |
| Enclosing scope chain (file → module nesting → type nesting) | **yes** — two modules may hold the same name (module scope L7233, nesting L7350, per-scope uniqueness L7264) | L7232–7233, L7350 |
| Member name / operator symbol / conversion target | yes (operators by §4.9; conversions by target type L8261) | L7824, L8260–8261 |
| Parameter types in **declaration order** | yes — parameter list = count/order/type L7794 | L7794, L7825 |
| Return type | **no** — MUST NOT participate (L7798); return-only overloads are duplicates (example L7860–7863) | L7798 |
| `&` by-reference marker | **no** — not part of the type / signature (L7551, L7812); encode by-refness only in ABI metadata, never in the mangled name (else two names for one signature) | L7551, L7812 |
| Parameter names, default expressions | **no** (L7813) | L7813 |
| Calling convention | **no** for in-language symbols (L7818–7820); external `linkable` names come from XREF §5.5 | L7816–7820 |
| `#` marker | **yes** — participates in signature (L7814) | L7814 |
| `#` **actual value** | **yes** — functions are monomorphized per `#` actual argument (L7544), so `f<#2>` and `f<#3>` are distinct symbols | L7544 |
| Generic actual arguments (ordered) | **yes** — one entity per monomorphization; static members attach per instantiation (L8678) | L8849, L8678, L8930–8936 |
| Empty-type actual argument | **yes** — must be distinguishable from arity mismatch and from other types (L8931) | L8931 |
| `this` parameter type (methods) | **yes** — method identity allows same name iff `this` type differs (L7270–7271); receiver slot position per L7686/L7825 | L7269–7271 |
| `this` excluded from method parameter list | — method list = list minus `this` (L7696) | L7696 |
| Accessibility / `readonly` / `verbatim` / `manual` / `scoped` / `virtual` / `override` / `sealed` / `static` | **no** — none is in a signature; including them would defeat L7264 duplicate detection | L7935, L7992 |

Consequence: **one mangled name per (scope path, name, ordered parameter types, `#` marks and values, generic actual arguments, `this` type)**; everything else must be erased. Add a stable escaping scheme so nested `<…;…>` arguments (and the `>>` lexing issue, OQ17) cannot alias.

### 7.2 Calling convention and parameter passing

- Default mode is **by value**; the only other mode in the range is **by reference** via `&` (L7549–7550). There is **no** `ref`/`out`/`in` and **no** variadic/`params` form (see §3.7); `wildcard` (XREF §3.6.3) is a function-type wildcard, not a variadic parameter.
- `&` is erased from source types (L7551) but changes the ABI: lower `&T` as a pointer (or LLVM `byref`-style parameter) and load/store through it; the callee sees the caller's storage, so no copy-in/copy-out.
- **Value-type `this`** `[L7687]` ⇒ pointer to the instance (by-ref receiver); **reference-type `this`** `[L7688]` ⇒ handle/pointer passed by value.
- **Receiver slot**: methods are called through member access `[L7690]` yet `this` may sit at **any** declared position `[L7686]` and "MUST NOT be moved" `[L7825]`. Therefore the LLVM argument order is the **declared** parameter order including `this`; the call site supplies the receiver for the `this` slot and the remaining arguments for the other slots in order (or by name). Do not silently reorder `this` to position 0. (Call-site binding is OQ7.)
- `#` parameters are compile-time only: they **MUST NOT** be accessed at runtime `[L7545]` and therefore **must not appear** in the LLVM runtime signature — but they do multiply *instantiations* and *symbol names* `[L7544]`.
- Static members take **no receiver** `[L8676–L8677]`; `static fixed Construct()` is a plain zero-argument function `[L8740]`.
- Because calling convention is not part of the signature `[L7820]` and CC-only variants may not coexist `[L7818]`, all in-language functions of one signature share a single ABI; a different CC must be confined to `linkable` (XREF §5.5).
- Overload resolution is fully static `[L7796]` ⇒ direct calls everywhere except `virtual`/`override` dispatch; explicit `Cast` at the call site `[L7807]` is the only source-level disambiguator.

### 7.3 Static initialization and data placement

- A static field whose initializer is compile-time determinable **MUST** be emitted into a **writable section** `[L8679]`; otherwise the **Launcher** initializes it at runtime `[L8679]` (XREF §14.2). Note "writable" is explicit — even though a `static readonly` field is immutable after init, the spec does not say `.rodata` (OQ; the example `public static readonly Instance: Singleton;` is uninitialized at L7948 and initialized elsewhere).
- Static constructors (`Construct`, void, `static` `[L8683]`) **MUST** run before `Main` `[L8684]`; they are the only in-range initialization hook (modules explicitly have none, NOTE L8697–8700). Ordering/cycles are OQ22.
- Generic type static members attach to the **monomorphized** type `[L8678]` ⇒ one global per instantiation, initialized per instantiation; mangled global names must include the type-argument tuple.
- A `verbatim` constant/variable initializer must **not** be compile-time evaluated `[L8487]` ⇒ cannot be folded into `.rodata`; force runtime initialization even for literal arithmetic (example L8500–8503).
- `manual` heap units **MUST NOT** be auto-released by NOLAA `[L8524]` ⇒ emit no scope-exit free; only explicit `dispose` `[L8525]` lowers to the deallocation call.
- Modules produce **no runtime entity** `[L7355]`; module-scope `local`/`fixed` are plain globals with module-qualified names, not a struct/instance.
- Type-level accessors lower to the **Backing** (same-type static field, global variable, or global constant) `[L8695]`; no storage of their own.

### 7.4 Virtual dispatch

- `virtual` methods **MUST** have a body `[L8575]`, so every virtual slot starts with an implementation; there are **no** abstract methods in this range (`abstract` only modifies classes `[L8539]`), so pure-virtual slots can only originate from interfaces (XREF §3.7.3).
- `override` must satisfy the class override requirements `[L8589]` and **MAY** covariantly replace the `this` target type `[L8591]` ⇒ reuse the base slot; if the covariant receiver needs an adjustment, emit a thunk. For `structure` members `override` is forbidden `[L8590]` and `virtual` is forbidden `[L8574]`, so value types never dispatch.
- `sealed` on a class ⇒ no subclasses; `sealed` on an override ⇒ no further overrides `[L8557–8559]` ⇒ devirtualize calls bound at or below that point.
- Methods **MUST NOT** enter the target's memory layout `[L7692]`; whether a vtable **pointer** is stored in instances is not stated in the range (it is not a method) — instance-header / vtable layout belongs to XREF §3.7.1. Interface dispatch is out of range.
- Overload resolution selects exactly one method at compile time `[L7796, L7825]`; only the dispatch step is dynamic. `static` members are never virtual (`static` + `this` forbidden `[L8669]`).
- `scoped`/accessibility restrictions are compile-time only and must not leave any runtime check.

### 7.5 Monomorphization naming

- Monomorphization is compile-time only `[L8848, L8941]`; one instance per distinct ordered type-argument tuple `[L8849, L8930–8936]` **and** per distinct `#` actual value `[L7544]`.
- Symbol names must be deterministic and content-addressed by: declaration's mangled base name + ordered type args + ordered `#` values (L7544, L8678). Include empty-type placeholders `[L8931]` and nested generic arguments recursively.
- Static members of generic types get **per-instance globals** `[L8678]`; accessors that borrow outer generic parameters `[L8844]` inherit the enclosing instance's arguments in their symbol.
- Failure (arity/constraint) is a **compile error**, not a codegen fallback `[L8851, L8942, L8936–L8937]`.
- §14.3.2 volume optimization `[L8949]` may merge identical bodies, but symbols must remain per-instantiation because static storage is per-instantiation `[L8678]`; do not deduplicate globals.
- No type-argument inference exists in the range (the count must be written `[L8936]`), so every instantiation site is syntactically explicit; internal instantiations (e.g. a generic type field of type `T`) still need derived names.

### 7.6 Modifier → codegen effect matrix

| Modifier | Codegen effect (derived) | Must not |
| --- | --- | --- |
| `public` / `private` / `protected` / `scoped` | **none** — erased; diagnostics only `[L7992, L8011]` | leak into symbols; leave runtime checks |
| `readonly` | one-time initialization; later stores rejected; assignment allowed in ctor `[L8385]` | be re-assigned after init |
| `implicitable` | none at runtime; selects implicit conversion at compile time `[L8404]` | add a runtime conversion hook |
| `verbatim` | disable folding/evaluation; preserve source structure `[L8485–L8487]`; ABI/regalloc still allowed `[L8489–8491]` | change call/stack layout because of it |
| `manual` | heap unit not auto-freed; `dispose` required `[L8524–L8525]` | emit NOLAA scope-exit free |
| `abstract` | no construction; inherited `Construct`/`Destruct` promoted `[L8540–8543]` | emit an instantiation path |
| `sealed` | final class / final override ⇒ devirtualize `[L8557–8559]` | create a vtable slot beyond a sealed override |
| `virtual` | vtable entry + indirect dispatch `[L8573–L8575]` | omit the body `[L8575]` |
| `override` | reuse base slot; covariance thunk if needed `[L8589–L8591]` | modify `structure` members `[L8590]` |
| `aligned(N)` | `align N` on struct/field; union member ⇒ explicit byte offset padding `[L8609–L8620]` | adjust union offsets to platform alignment `[L8620]` |
| `static` | global storage, no receiver, per-instantiation for generics, init per §7.3 `[L8676–L8679]` | enter instance layout `[L8676]`; use instance access `[L8677]` |

### 7.7 Residual codegen risks (tie to OQ)

- By-refness is not part of the type `[L7551]` yet changes the ABI; keep an ABI-only flag, or `&`-variants will collide in the symbol table (they must: `&` cannot be overloaded, `[L7812]`).
- Receiver position when `this` is not first `[L7686, L7825]` — must be preserved in LLVM parameter order (OQ7).
- `aligned(0)`/non-pow2 alignment `[L8613]` — define lowering (OQ14); software emulation is required `[L8614, L8617]`.
- `verbatim` granularity — block-level vs function-level optimization suppression (OQ18); a block may need outlining or an optimization barrier to satisfy L8485–L8487 without violating L8489–L8491.
- Static-init order and multiplicity `[L8679, L8684]` (OQ22); `.data` vs `.rodata` for `static readonly` `[L8679]`.
- Vtable presence/instance header for classes `[L7692]` vs virtual dispatch `[L8573]` — not specified in the range; vtable/instance-header layout belongs to XREF §3.7.1 (see §7.4).
- Monomorphization `<…>` parsing / `>>` splitting `[L8921, L8931]` (OQ17) — affects both mangling stability and diagnostic quality.
- Duplicate detection: two declarations differing only by accessibility/`static`/`verbatim` must be a compile error `[L7264, L7935]`, not two symbols (OQ20).
