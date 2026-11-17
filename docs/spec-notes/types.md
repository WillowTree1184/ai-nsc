# NovaSharp 类型系统 — 实现笔记（Draft 1.1 §3，spec lines 1456–3539）

Source: `docs/grammar/Draft 1.1.md`, §3 类型, lines **1456–3539** (read in full: 2084 lines, 16 `ebnf`
blocks, 16 `novasharp` example blocks). Out-of-range material is marked `[XREF]` and used only to
disambiguate or to record a dependency.

Tags: each normative bullet ends with its spec line range, e.g. `[L2124-2131]`. `MUST` = 必须,
`MUST NOT` = 不得, `MAY` = 允许, `SHOULD` = 应当. Keywords/identifiers keep the spec spelling
(`Construct`, `Destruct`, `Cast`, `aligned`, `wildcard`, `get`, `set`, `@base`, `#`, `&`, `=>`).

---

## 1. Section map (line anchors)

| Section | Title | Lines | Content |
| --- | --- | --- | --- |
| §3 | 类型 | 1456–1457 | heading |
| §3.1 | 类型构造语法 | 1458–1507 | `Type`, prefix/right-assoc, generics-in-type ban |
| §3.2 | 基本类型 | 1508–1635 | `integer(N)`/`cardinal(N)`/`float(N)`/`boolean`, N, builtin overloads, behavior, casts |
| §3.3 | 值类型 | 1636–1648 | value type, copy-on-assign |
| §3.4 | 引用类型 | 1649–1662 | reference type, copy the reference |
| §3.5 | 引用 | 1663–1712 | reference = implicit pointer; aliasing; no explicit deref; no ref-to-ref |
| §3.6 | 类型构造器 | 1713–1755 | `ConstructedType`, constructors, target, empty type |
| §3.6.1 | 数组类型构造器 | 1756–1859 | arrays, fixed vs VLAs, casts, sugar |
| §3.6.2 | 指针类型构造器 | 1860–1912 | pointer target/casts/arithmetic |
| §3.6.3 | 函数类型构造器 | 1913–2079 | function types, `wildcard`, defaults, calling convention, equality/casts |
| §3.7 | 自定义类型 | 2080–2081 | heading |
| §3.7.1 | 类 | 2082–2312 | class, layout, lifetime, ctor/dtor, inheritance, override, dispatch, `Object` |
| §3.7.2 | 结构体 | 2313–2472 | struct, alignment/layout, init order, ctor/dtor |
| §3.7.3 | 接口 | 2473–2630 | interface contracts, implementation, empty interface, inheritance, casts |
| §3.7.4 | 枚举 | 2631–2761 | enum, underlying type, values, layout, casts, name elision |
| §3.7.5 | 变体 | 2762–2914 | states/payloads, tag+payload layout, lifetime, equality, name elision |
| §3.7.6 | 共同体 | 2915–3042 | union offsets, construction, access, name elision |
| §3.7.7 | 继承列表 | 3043–3094 | `BaseList` and per-declaration constraints |
| §3.8 | 成员 | 3095–3096 | heading |
| §3.8.1 | 字段 | 3097–3175 | fields, init/inference, function fields |
| §3.8.2 | 访问器 | 3176–3413 | `get`/`set`, shorthand, Backing Field, interface/type-level |
| §3.8.3 | 扩展 | 3414–3539 | extension targets (type/generic/array), members, interfaces, visibility |

Adjacent context: §2.5, §2.6, §2.7.9, §4, §5, §8.2, §9, §10, §12.1, §14.3, §15.2 (see §8).

---

## 2. Consolidated EBNF (verbatim, source order)

### 2.1 §3.1 类型构造语法 — L1462-1464

```ebnf
Type = ConstructedType | BasicType | Identifier | MonomorphizationExpression .
```

### 2.2 §3.2 基本类型 — L1512-1522

```ebnf
BasicType = IntegerType
          | CardinalType
          | FloatType
          | BooleanType .

IntegerType         = "integer" "(" IntegerLiteral ")" .
CardinalType        = "cardinal" "(" IntegerLiteral ")" .
FloatType           = "float" "(" IntegerLiteral ")" .
BooleanType         = "boolean" .
```

### 2.3 §3.6 类型构造器 — L1717-1725

```ebnf
ConstructedType = TypeConstructor [ ConstructTargetType ] .

TypeConstructor = ArrayTypeConstructor
                | FunctionTypeConstructor
                | PointerTypeConstructor .

ConstructTargetType = BasicType | Type .
```

### 2.4 §3.6.1 数组类型构造器 — L1760-1762

```ebnf
ArrayTypeConstructor = "array(" Expression { "," Expression } ")" .
```

### 2.5 §3.6.2 指针类型构造器 — L1864-1866

```ebnf
PointerTypeConstructor = "pointer" .
```

### 2.6 §3.6.3 函数类型构造器 — L1917-1923

```ebnf
FunctionTypeConstructor = "function" [ "(" [ ParameterList ] [ ";" CallingConventionExpression ] ")" ] .

ParameterList = Parameters [ "," "wildcard" ] | "wildcard" .

CallingConventionExpression = Expression .
```

### 2.7 §3.7.1 类 — L2086-2088

```ebnf
ClassDeclaration = { Modifier } "class" Name [ GenericParameters ] [ BaseList ] Block .
```

### 2.8 §3.7.2 结构体 — L2317-2319

```ebnf
StructureDeclaration = { Modifier } "structure" Name [ GenericParameters ] Block .
```

### 2.9 §3.7.3 接口 — L2477-2479

```ebnf
InterfaceDeclaration = { Modifier } "interface" Name [ GenericParameters ] [ BaseList ] Block .
```

### 2.10 §3.7.4 枚举 — L2635-2639

```ebnf
EnumerationDeclaration = { Modifier } "enumeration" Name ":" BasicType "{" { EnumerationMemberDeclaration | ExtensionDeclaration } "}" .

EnumerationMemberDeclaration = Name "=" Expression ";" .
```

### 2.11 §3.7.5 变体 — L2766-2770

```ebnf
VariantDeclaration = { Modifier } "variant" Name [ GenericParameters ] "{" { VariantMemberDeclaration | ExtensionDeclaration } "}" .

VariantMemberDeclaration = Name [ ":" Type ] ";" .
```

### 2.12 §3.7.6 共同体 — L2919-2923

```ebnf
UnionDeclaration = { Modifier } "union" Name [ GenericParameters ] "{" { UnionMemberDeclaration | ExtensionDeclaration } "}" .

UnionMemberDeclaration = { Modifier } Name ":" Type ";" .
```

### 2.13 §3.7.7 继承列表 — L3047-3049

```ebnf
BaseList = ":" ( Identifier | MonomorphizationExpression ) { "," ( Identifier | MonomorphizationExpression ) } .
```

### 2.14 §3.8.1 字段 — L3101-3103

```ebnf
FieldDefinition = { Modifier } Name [ ":" Type ] [ "=" Expression | Block ] .
```

### 2.15 §3.8.2 访问器 — L3180-3190

```ebnf
AccessorDefinition        = { Modifier } "accessor" Name ":" Type [ "=>" Backing ] [ AccessorMembers ] .

AccessorMembers           = "{" [ AccessorMemberDefinition { ";" AccessorMemberDefinition } [ ";" ] ] "}" .

AccessorMemberDefinition  = { Modifier } ( "get" | "set" ) [ ":" Type ] [ "=" Expression ]
                          | { Modifier } "get" "(" [ "this" ":" "&" Type ] ")" Block
                          | { Modifier } "set" "(" [ "this" ":" "&" Type "," ] "value" [ ":" Type ] ")" Block .

Backing                   = Identifier .
```

### 2.16 §3.8.3 扩展 — L3418-3430

```ebnf
ExtensionDeclaration = "extension" [ "for" ExtensionTarget ] Block .

ExtensionTarget      = Type
                     | GenericTarget
                     | ArrayTarget .

GenericTarget        = Identifier "<" GenericParameterDefinition { ";" GenericParameterDefinition } ">" .

ArrayTarget          = "array" "(" TargetBinding { "," TargetBinding } ")" GenericParameterDefinition .

TargetBinding        = Name ":" Type .
```

### 2.17 Referenced productions (out of range, quoted)

```ebnf
# §8.2 参数 — L7497
Parameters        = Parameter { "," Parameter } .
Parameter         = [ "#" ] [ Name ":" ] [ "&" ] Type [ "=" Expression ] .
LiteralParameters = LiteralParameter { "," LiteralParameter } .
LiteralParameter  = [ "#" ] Name [ ":" [ "&" ] Type ] .
```

```ebnf
# §10.1 泛型参数 — L8784
GenericParameters = "<" GenericParameterDefinition { ";" GenericParameterDefinition } ">" .
GenericParameterDefinition = GenericParameter [ ":" Constraint { "," Constraint } ] .
GenericParameter = Identifier .
Constraint       = TypeKind | Identifier .
TypeKind = "class" | "structure" | "interface" | "enumeration" | "variant" | "union" | "basic" .
```

```ebnf
# §10.2 单态化表达式 — L8916
MonomorphizationExpression = ( Identifier | Expression ) "<" [ Type ] { ";" [ Type ] } ">" .
```

Generic arguments use `<...>`, separated by `;`: `Box<integer(4)>`, `Result<T; E>`, `Outcome<integer(4)>`
(L1501, L2895); empty generic args mean the empty type [XREF L8931].

---

## 3. Semantic rules

### 3.1 Type construction (§3.1)

- `Type` is exactly a constructed type, basic type, `Identifier`, or `MonomorphizationExpression` [L1463].
- A type `Identifier` MUST resolve to a type [L1470-1472].
- Construction is **prefix and right-associative**: constructor first, target after; nesting goes right
  [L1477-1478].
- Generic **parameters** MUST NOT appear in a type; only in declarations [L1482]. Generic **arguments**
  MUST be carried by a monomorphization expression [L1483].
- `<T> function(value: T) T` is illegal [L1503-1505].
- No other type syntax: `&`, `*`, `[]` and postfix forms are not `Type` (§3.5: references are not a type
  constructor; `&` exists only in parameter lists) [L1663-1673, L1918-1920].

### 3.2 Basic types (§3.2)

- `integer(N)` / `cardinal(N)`: N MUST be `1`, `2`, `4`, `8`, `16` [L1529, L1533]; `float(N)`: N MUST be
  `4` or `8` [L1536]. Unsupported N MUST be emulated, not rejected [L1530, L1534, L1537].
- Parentheses MUST NOT be omitted; without them the spelling is not a basic type [L1539-1540]. Since
  `integer` etc. are keywords [XREF L519], `local bad: integer = 42;` cannot fall back to `Identifier`
  and is an error [L1615-1617].
- N is in **bytes**; memory size of integer/cardinal/float is exactly N [L1542-1545]. `cardinal(N)` range
  is `0 .. 2^(8N)-1` [L1532]. `boolean` size MUST be 1 byte [L1554].
- Floats MUST follow IEEE 754 [L1549]; operation precision MUST NOT exceed the operand type's precision
  (no 80-bit intermediates) [L1550].
- Builtin overloads: for `integer(N)` and `cardinal(N)` all five N, **all eight binary operators plus
  unary negation** [L1558-1560]; same for `float(4)`/`float(8)` [L1561-1562]; for `boolean`, **only**
  equality, all other operators forbidden [L1563].
- Arithmetic builtin signature MUST be `function(left: T, right: T) T` [L1564]; comparison MUST be
  `function(left: T, right: T) boolean` [L1565].
- "Eight binary operators" is not enumerated in §3.2; §4.9.2's table has exactly eight binary entries
  (`+`, `-`, `*`, `/`, `%`, `==`, `>`, `<`) [XREF L4592-4601]. Inferred; see OQ-4.
- Behavior: integer overflow **wraps** [L1569]; division truncates toward zero [L1570]; remainder sign
  follows the **left** operand [L1571]; division/remainder by `0` MUST raise `DivideByZeroException`
  [L1572]; logical shift counts are taken **modulo the left operand type's bit width** [L1573]; float
  rounding is round-half-to-even [L1574]; float comparison is IEEE (NaN unequal to everything, incl.
  itself) [L1575]; float division is IEEE (÷0 gives inf/NaN, MUST NOT raise `DivideByZeroException`)
  [L1576]; integer/cardinal negation wraps [L1577]; float negation is IEEE, `-infinite` = negative
  infinity [L1578].
- Conversions: basic↔basic MUST be numeric [L1582]; **no implicit** basic↔basic conversion [L1583];
  explicit both ways allowed [L1584]; float→integer out of range MUST wrap [L1585]; →`boolean`: `0`→
  `false`, everything else→`true` [L1586]; from `boolean`: `false`→`0`, `true`→`1` [L1587].
- Signed range of `integer(N)` is never stated (see OQ-6).
- Illegal examples: `integer(3)`, `float(16)` [L1619-1625]; implicit int→float, int→bool [L1627-1633].

### 3.3 Value types, reference types, references (§3.3–§3.5)

- **Value type**: the identifier directly holds the instance body; assignment MUST copy the body
  [L1638-1646]. Value types in range: array [L1758], pointer [L1862], structure [L2315], enumeration
  [L2633], variant [L2764], union [L2917].
- **Reference type**: the identifier stores a reference; assignment MUST copy the reference [L1651-1659].
  Reference types in range: class [L2084], function [L1915]. Interface is never classified (OQ-1).
- Reference = **implicit pointer**; in layout a pointer to the instance body [L1669-1671].
- Syntactically a reference **is** the instance body and its type **is** the instance type [L1672-1673];
  therefore no reference type syntax and no deref operator.
- Multiple references to one instance are allowed (aliasing) [L1674].
- A reference to a reference-type instance points directly at the body, never at another reference
  [L1678]. There is **no reference-to-reference**; only a pointer to a reference [L1680-1682].
- Every operation on a reference maps to an operation on the body [L1686]; explicit dereference is
  forbidden (`(*counter).Count = 5;` illegal) [L1687, L1708-1710].
- `&` is not a type constructor: it is a by-reference parameter marker [XREF L7499] and appears in
  accessor `this` shorthand [L3186-3187, L3246].

### 3.4 Constructors and the empty type (§3.6)

- `ConstructedType = TypeConstructor [ ConstructTargetType ]`; target follows the constructor
  [L1717-1718].
- `TypeConstructor` MUST be one of the three constructors [L1737].
- Arrays MUST carry a target; function and pointer constructors MAY omit it [L1744-1745].
- **Empty type** = type obtained when `ConstructTargetType` is omitted [L1732]. It has: no layout
  [L1746]; no written form [L1747]; MUST NOT be a variable type [L1748] or a field type [L1749]; is used
  **only** for function return, pointer target and variant state payload [L1750].
- An accessor MUST NOT be a constructor's target type [L1752-1754] (accessors are not types).
- No general subtyping lattice exists for value types. The only subtype-like relations are: derived
  class→base class, class→implemented interface, sub-interface→base interface, enum→underlying type.
  Arrays and function types use structural equality; user types are nominal.

### 3.5 Array type constructor (§3.6.1)

- `array(` size-expr {`,` size-expr} `)` followed by the element type [L1761].
- Size-expression type MUST be `cardinal` **or implicitly convertible to** cardinal [L1767] — an explicit
  exception to L1583 (OQ-5).
- N = size-expression value = array size [L1768-1769]; element type = `ConstructTargetType` [L1773].
- Size MUST be compile-time determinable, **except** when the constructor occurs inside a function body
  [L1774-1775]. The size is compile-time metadata and MUST be stripped from output artifacts
  [L1776-1777] (applies to fixed arrays; OQ-7).
- **Fixed array**: size is a compile-time non-negative integer and is stripped as metadata [L1781-1783].
  Negative size illegal (`array(-1) ...`) [L1855-1857].
- **VLA**: size not compile-time determinable [L1787]; only inside a function body [L1788]; MUST NOT be a
  return type [L1789]; when in a local definition it equals a struct of **pointer first, length second**
  [L1790-1792], the pointer MUST point at the first element [L1791], the length MUST be the size
  expression's value [L1792]; MUST NOT be a type-comparison target [L1793].
- **Type equality**: array `F` = array `L` iff same size AND same element type [L1801-1803].
- Conversions: array→pointer allowed one-way explicit, pointer target = element type, points at first
  element [L1807-1809]; pointer→fixed array allowed one-way explicit, same target rule, references the
  contiguous elements [L1810-1812]; pointer→VLA **forbidden** [L1813]; fixed↔fixed explicit both ways,
  size-unrestricted [L1814-1815]; VLA↔VLA explicit both ways [L1816]; VLA→fixed explicit one-way
  [L1817]; pointer→array MUST NOT check size and SHOULD warn [L1818].
- Sugar: `array(X) array(Y) T` ≡ `array(X, Y) T`, recursive [L1822-1824]. `array(Name: Type) T` ≡
  parameters `Name: Type` and `pointer T`, where `Type` MUST be cardinal, expansion order pointer then
  length, only in function type/function literal parameter lists, only the first dimension of
  multi-dimensional arrays [L1825-1830].
- Array definition shorthand is §2.5 [L1831, XREF L400-500].

### 3.6 Pointer type constructor (§3.6.2)

- `pointer` + optional target [L1865, L1718]; pointer is a **value type** [L1862]; size is
  platform-determined [L1871]; the target says how the pointed-to memory is accessed [L1875].
- Pointer↔pointer with different targets: explicit both ways, changes only the target type, MUST NOT
  change the address value [L1879-1880]; SHOULD warn [L1881].
- A target-less `pointer` MUST NOT be dereferenced [L1885, L1907-1910].
- Arithmetic builtins cover `+` and `-` [L1889]: pointer±integer→pointer [L1890]; pointer−pointer→element
  count [L1891]; unit = `Type.SizeOf<T>()` with `T` the target type [L1892]; target-less → 1 byte
  [L1893].

### 3.7 Function type constructor (§3.6.3)

- Shape: `function [ "(" [ ParameterList ] [ ";" CallingConventionExpression ] ")" ]` [L1918];
  `ParameterList = Parameters [ "," "wildcard" ] | "wildcard"` [L1920].
- `wildcard` is a keyword [L1933] matching any remaining actual arguments at the end [L1932]; it MUST be
  the last item and MUST NOT appear before/between fixed parameters [L1934, L1946].
- With `wildcard`, the preceding part is the **fixed parameter prefix**: the compiler MUST check the
  corresponding actual arguments [L1943] and MUST NOT check the remaining ones [L1944].
- `wildcard` MAY appear alone or after `Parameters` [L1945].
- Parameter names MUST appear unless useless in context; they MUST appear when the function body must
  bind them [L1947-1948].
- The calling-convention expression MUST NOT carry parameter names and MUST be separated from the
  parameters by `;`; with an empty parameter list the `;` MUST appear (`function(; CDeclaration)`)
  [L1949-1950, L2044].
- Calling convention MUST return `CallingConvention` and be compile-time evaluable [L1955-1957]; default
  is `CallingConvention.PlatformDefault` [L1958], which MUST mean the target platform default [L1959].
  `CallingConvention` is defined by the Built-In Library in global scope [L1960-1962]; members:
  `PlatformDefault`, `CDeclaration`, `StandardCall`, `FastCall`, `SystemVAMD64`, `ThisCall`, `VectorCall`,
  `MicrosoftX64` [XREF L10168-10187].
- Return type = target type [L1966]; MAY be omitted → empty type [L1967-1968]; when omitted, parentheses
  MUST NOT be omitted [L1969].
- Default value expressions are **caller behavior** and belong to the function type [L1936-1938]; only the
  function type constructor supports them, not function literals [L1937-1938]; a defaulted parameter MUST
  NOT be followed by a non-defaulted one [L2067-2069]; a default MUST NOT appear when the name is omitted
  [L2063-2065].
- **Type equality** iff **all**: fixed-prefix param count/order/types equal; `#` convention same per
  corresponding parameter; `wildcard` presence same; calling convention same; return type same
  [L1973-1978]. Parameter names, default expressions and `&` MUST NOT participate [L1979]; `#` MUST
  participate [L1980]; inferred types MUST preserve parameter names without changing equality [L1981].
- Conversions MUST only change the caller's calling behavior [L1985]; explicit between legal function
  types is allowed both ways, unrestricted by parameter list, calling convention or return type
  [L1986-1987]; implicit is **forbidden** [L1988]; explicit SHOULD warn [L1989].
- Sugar: parentheses may be omitted when the parameter list and calling convention are inferable **and**
  the return type is not omitted (`function integer(4)` ≡ `function() integer(4)`) [L1991-1993]. Array
  parameter expansion also applies [L1994].

### 3.8 Class (§3.7.1)

- A class is a **reference type**, has no default memory allocation way, supports single inheritance,
  multiple interfaces, polymorphism and method overriding [L2084]. Syntax: `{ Modifier } "class" Name
  [ GenericParameters ] [ BaseList ] Block` [L2087].
- Modifiers precede `class` [L2102] and MUST be only: accessibility, `sealed`, `abstract`, `static`
  [L2103-2107].
- Body MUST be a declaration-sequence body [L2108] and MUST directly contain only field definitions,
  extension declarations, and modules (modules only for identifier organization) [L2109-2112].
- Members MUST support only: accessibility; `virtual` (methods only); `override` (methods only);
  `static`; other modifiers the member kind supports [L2116-2121]. `local` variables are forbidden in the
  body [L2122].
- §3.7.1's member list omits `sealed`, although L2201 refers to sealed methods and §9.11 allows it
  (OQ-11).
- **Layout**: `static` members, methods and accessors MUST NOT enter the instance layout [L2126-2129].
  Every instance MUST carry type information and a virtual table [L2130]. Devirtualization and metadata
  minimization per §14.3.1 [L2131].
- **Lifetime**: instantiate = (1) allocate, (2) initialize members **top-down along the inheritance
  chain**, (3) call the constructor [L2135-2138]; destroy = (1) call the destructor, (2) reclaim memory
  [L2139-2141].
- **Constructor**: method named `Construct`, empty return type [L2145]. **Static constructor**: named
  `Construct`, empty return type, empty parameter list, `static` constant [L2146] (no `this`; §9.15
  L8683-8684).
- When the base constructor has a non-empty parameter list **or** a non-empty body: the constructor MUST
  call the base constructor as its **first statement** [L2147], and the class MUST NOT omit its
  constructor [L2148].
- Member initialization MUST follow the instance's allocation way; it MUST NOT specify an independent one
  [L2149].
- **Default-constructor sugar** — only when the class is not abstract, declares no constructor, and has no
  base constructor with non-empty parameters/body — creates an empty `Construct` with: empty return type,
  `this` type = the class, empty parameter list, `CallingConvention.PlatformDefault` [L2167-2171].
- **Destructor**: method named `Destruct`, empty return type, empty parameter list [L2153]; MAY throw
  [L2154]; if it throws it MUST terminate execution and no subsequent destruction instruction may run
  [L2155]; if it does not complete, heap storage MUST NOT be freed [L2156]; scope storage is released as
  the stack pointer is restored [L2157]. **Default-destructor sugar** analogously when no destructor is
  declared and the class supports one [L2172-2176].
- Constructors/destructors MUST be methods [L2161]; their only difference from ordinary methods is
  automatic invocation [L2162]; they have special identity only when the type can be constructed/destroyed
  [L2163].
- **Inheritance**: at most one class [L2180]; the target MUST be non-`sealed` [L2181]. `base` is the
  keyword for base members [L2183-2184]; if shadowed, `@base` MAY be used [L2185]; `base` MAY be omitted
  when unambiguous [L2186]; base members accessed up the chain MUST have base accessibility **≥
  `protected`** [L2187].
- Base list: interfaces MUST be implemented, classes inherited [L2191-2192]; implementing a **non-empty**
  interface needs one or more extension blocks jointly implementing all members [L2193]; the list MUST
  NOT contain the class itself [L2194]; generic classes MUST be monomorphized [L2195].
- **Override**: MUST use `override` [L2199]; conditions: no sealed override of the same method up the
  chain; same name; same type except covariant `this` target; behavior-limiting modifiers absent/equal/
  fewer; behavior-activating absent/equal/more; optimization absent/equal; accessibility ≥; `this` at the
  same position [L2200-2209].
- **Dispatch**: virtual call through a base/interface identifier runs the most specific override of the
  **actual** instance type [L2213]; `base.M()` runs the base implementation with **no** dynamic dispatch
  [L2214]; non-virtual call runs the method from the declared type [L2215].
- **Conversions**: reinterpretation only, no content change [L2219]; base↔derived explicit both ways
  [L2220]; derived→base implicit one-way [L2221].
- **`Object`**: special abstract class [L2225]; classes with no declared base implicitly inherit it
  [L2226]; its `base` holds a self-reference [L2227]; the Built-In Library MUST define it in global scope
  [L2228], empty body [XREF L10163-10166].

### 3.9 Structure (§3.7.2)

- Value type, no default allocation way [L2315]; syntax `{ Modifier } "structure" Name
  [ GenericParameters ] Block` [L2318].
- Modifiers precede `structure` [L2331] and MUST be only accessibility + `aligned(N)` [L2332-2334].
- MUST NOT carry a base list [L2335]. Body = declaration-sequence body [L2336] directly containing only
  fields, extensions, and modules [L2337-2340].
- Members support accessibility, `aligned(N)`, `static`, and member-kind modifiers [L2344-2348]; MUST NOT
  be `virtual` [L2349]; MUST NOT define variables [L2350].
- **Layout**: `static` members, methods and accessors excluded [L2354-2357]; NO vtable/RTTI [L2358];
  layout MUST follow **member definition order strictly** [L2359]; `aligned(N)` changes only default
  alignment/offset computation, never order [L2360].
- Alignment: with `aligned(N)`, all fields default to N-byte alignment; otherwise each field defaults to
  **alignment equal to the member type's size**; a field `aligned(M)` MUST use M bytes, overriding the
  struct default; N = 0 → all fields 0-byte aligned = compact layout [L2361-2365].
- Size = minimal to contain all members, and MUST be a multiple of every member's **non-zero** alignment
  [L2366-2367]. Arrays inside a struct are fixed-size inline arrays with contiguous elements
  [L2368-2369].
- Lifetime: allocate → initialize members → call constructor [L2373-2376]; call destructor → reclaim
  [L2377-2379].
- Init: evaluate each member initializer and assign it [L2383]; the compiler MUST reorder initialization
  if needed and MUST warn when the reordering fails [L2384-2385]; member init follows the instance's
  allocation way [L2386].
- `Construct` = method with empty return type [L2390]; `Destruct` = method with empty return and empty
  parameters, behavior as §3.7.1 [L2394-2395]; default-ctor/dtor sugar when none declared
  [L2399-2408].

### 3.10 Interface (§3.7.3)

- An interface is an API contract over extension content and MUST be implemented only by `class`
  [L2475]; syntax `{ Modifier } "interface" Name [ GenericParameters ] [ BaseList ] Block` [L2478].
- Modifiers precede it and MUST be accessibility only [L2493-2494]. Body MUST be a declaration-sequence
  body [L2495] and MUST NOT contain extension declarations — members are declared directly as contracts
  [L2496].
- Interface members are implementation contracts and produce **no runtime entity** [L2500].
- Body MAY contain only: constant definitions (`fixed`, **not initialized**), accessor definitions,
  operator definitions, and modules (identifier organization only; module bodies MUST NOT declare fields)
  [L2501-2505].
- Members MUST NOT be `static` [L2506]. Members MUST explicitly declare `public`; omitting it MUST be an
  error [L2507-2508]. Exception: operator contracts are implicitly `public` and MUST NOT carry an
  accessibility modifier [L2509].
- **Implementation**: interfaces MUST NOT be instantiated [L2513, L2594-2596]; the implementing class MUST
  list the interface in its base list [L2514] and MUST provide one or more extension blocks in its type
  body [L2515]; all extension-block members in the body MUST jointly implement all interface members;
  members outside extension blocks MUST NOT implement it; an **empty interface** requires none
  [L2516-2518]. External extensions MUST NOT implement interfaces [L2519].
- Implementing members MUST satisfy: same name; same kind; same type except covariant `this` target;
  behavior-limiting absent/equal/fewer; behavior-activating absent/equal/more; optimization absent/equal;
  accessibility ≥; method `this` at the same position; accessor has same-or-more members; accessor member
  accessibility ≥ interface accessor member [L2520-2531].
- **Empty interface** = itself and all inherited interfaces have no members [L2535]; it MUST NOT force
  extension blocks [L2536, L2590-2592].
- Inherited interface members become current members; an instance implementing the current interface MAY
  be carried by the inherited interface type [L2540-2542]. The base list MUST NOT contain the interface
  itself [L2543]; generic interfaces MUST be monomorphized [L2544].
- **Conversions**: interface↔class reinterpretation only, no content change [L2548]; class↔interface
  explicit both ways [L2549]; class→interface implicit one-way [L2550]; interfaces in an inheritance
  relation explicit both ways [L2551]; sub-interface→parent implicit one-way [L2552].
- §14.3 requires type info for interface conversions, which sits uneasily with "reinterpretation"
  (OQ-23).

### 3.11 Enumeration (§3.7.4)

- Value type mapping identifiers to underlying-type values [L2633]; syntax `enumeration Name ":" BasicType
  "{" { member | extension } "}"` [L2636]; member `Name "=" Expression ";"` [L2638].
- Modifiers precede `enumeration`, accessibility only [L2654-2655]. The underlying type MUST NOT be
  omitted and MUST be a basic type [L2647-2648, L2656]. No generic parameters [L2657, L2754-2759].
- Body = type body [L2658]; MAY contain extension blocks [L2659].
- Members MUST be **kindless** member definitions [L2663]; value expressions MUST NOT be omitted [L2664],
  MUST be compile-time constants [L2665], MUST NOT be duplicated [L2666].
- Layout MUST be identical to the underlying type's value layout [L2670]; the value MUST directly store
  the underlying-type value [L2671].
- Conversions: underlying↔enum explicit both ways [L2675]; enum→underlying implicit one-way [L2676].
- **Name elision**: when the target type is an enum and is contextually given, `Type.Name` MAY be written
  `Name` [L2680]. Contexts, exactly: explicitly typed variable/constant/field definitions; explicitly
  typed heap-allocation definitions; assignment/compound-assignment target; parameters of
  functions/methods/operators/function literals; their return types; parameter default values; the target
  type of `Cast<T>`; the right-hand side of `??`; the match target of `match`; the member value expression
  of a member construction [L2681-2691].
- Conditions: `Name` MUST be an enum member of `Type` [L2692]; no same-named binding in the current scope
  [L2693]; current-type members are in scope and a same-named `this` member MUST use the full form
  [L2694]; then `Name` MUST resolve to a member access [L2695]; with a same-named binding, write
  `Type.Name` [L2696]; other positions MUST NOT use `Name` [L2697]; in `match` value-equality patterns the
  target type MUST be the matched expression's type [L2698]; the full form is always allowed [L2699].

### 3.12 Variant (§3.7.5)

- Value type representing different states, no default allocation way [L2764]; syntax `variant Name
  [ GenericParameters ] "{" { state | extension } "}"` [L2767]; state `Name [ ":" Type ] ";"` [L2769].
- Modifiers precede `variant`, accessibility only [L2783-2784]. Body = type body [L2785], MAY contain
  extensions [L2786].
- A **state** is a member; a **payload** is the value it carries [L2790-2791]. A state MUST have zero or
  one payload [L2792]; omitting `:` means no payload [L2793].
- No-payload state MUST behave as a variant instance of that state [L2794]; payload state MUST be
  constructed via a member construction expression [L2795, XREF L4115-4130].
- A no-payload state and an enum member are syntactically identical and both resolve to a member access;
  semantically an enum member is an underlying value and a no-payload state is a variant instance
  [L2797-2799].
- **Layout**: the **state space** is an enum over all states, whose underlying type MUST be the smallest
  integer type sufficient for the number of states [L2803-2804]; the **payload space** is a union of all
  payload types [L2805]; layout MUST be state space first, payload space after [L2806]; each MUST be
  aligned "according to its own size" [L2807] (ambiguous; OQ-13).
- **Lifetime**: the state MUST be determined at instantiation [L2811, L2902-2904]. Assigning a new state
  MUST modify the state and payload spaces [L2812]; if the old state carried a payload, destroy it
  **before** the modification [L2813]; the modification MUST NOT create a new instance [L2814].
- **Equality**: `F` = `L` iff same type, same state, and (if any) equal payloads [L2818-2821]; payload
  equality follows the payload type's equality semantics [L2822].
- **Name elision**: when contextually typed, `Type.Name`→`Name` and `Type.Name(Expression)`→
  `Name(Expression)` [L2826]; contexts = the enum list **plus** the left operand of `is` [L2827-2838];
  no-payload MUST be `Name`, payload MUST be `Name(Expression)` [L2839]; `Name` MUST be a state [L2840];
  no same-named binding [L2841]; same-named `this` member needs the full form [L2842]; then `Name` MUST
  resolve to member access and `Name(Expression)` to member construction, NOT a function call [L2843];
  with a same-named binding **or function**, write the full form [L2844]; no short form elsewhere
  [L2845]; in `is` the short form's target type MUST be the left operand's type and in `match` the match
  target's type [L2846]; full form always allowed [L2847].

### 3.13 Union (§3.7.6)

- Value type giving different views of one memory block, no default allocation way [L2917]; syntax
  `union Name [ GenericParameters ] "{" { member | extension } "}"` [L2920]; member `{ Modifier } Name ":"
  Type ";"` [L2922].
- Modifiers precede `union`, accessibility only [L2936-2937]. Body = type body [L2938], MAY contain
  extensions [L2939].
- Members MUST be kindless member definitions [L2943] and MUST support **only** `aligned(N)` [L2944]
  (`public` on a union member is illegal [L3035-3040]); the modifier list precedes the name [L2945];
  members MUST NOT carry an initial value [L2946, L3028-3033]; a union MUST NOT contain accessors
  [L2947].
- **Construction**: MUST go through a member construction expression [L2951]; the **constructed member**
  is the specified one, the rest are **unconstructed** [L2952-2953]; the constructed member MUST be
  initialized to the member value expression's result [L2954]; the expression MUST initialize **only**
  the constructed member and MUST NOT initialize unconstructed members [L2955].
- **Layout**: base start point = the union instance's memory start [L2959]; members use it as the offset
  base [L2960]; a member offset is relative to it [L2961]; every member MUST have an offset [L2962]:
  without `aligned(N)` it MUST be `0` [L2963], with `aligned(N)` it MUST be `N` [L2964]; union size =
  max over members of (offset + member size) [L2965].
- **Access**: reads start at the member offset and interpret the memory as that member's type [L2969];
  writes start at the offset and write as that type [L2970]; unconstructed members MAY be read and
  written (type punning allowed) [L2971].
- **Name elision**: contextually typed `Type.Name(Expression)` MAY be `Name(Expression)` [L2975]; the
  context list is the enum list (10 entries, no `is`) [L2976-2986]; `Name` MUST be a member [L2987]; no
  same-named binding [L2988]; same-named `this` member needs the full form [L2989]; then `Name(Expression)`
  MUST resolve to member construction, NOT a function call [L2990]; with a same-named binding **or
  function**, write the full form [L2991]; no short form elsewhere [L2992]; **reads MUST NOT use the
  short form** and MUST go through an instance member access [L2993]; in `match` value-equality patterns
  the target type MUST be the matched expression's type [L2994]; full form always allowed [L2995].

### 3.14 Base list (§3.7.7)

- `:` + one or more `Identifier`/`MonomorphizationExpression` targets separated by `,` [L3048]; MUST
  start with `:`; targets MUST be comma-separated [L3059-3060].
- In a class: at most **one class** + multiple interfaces [L3061]. In an interface: **only interfaces**
  [L3062]. Only a class's base list may contain interfaces; an interface's MUST NOT contain a class
  [L3063].
- Interfaces in a base list MUST be implemented by extensions in the class body [L3064]. The list MUST
  NOT contain the current type itself [L3065].
- Additional constraints: base class must be non-`sealed` [L2181]; interfaces MUST be implemented
  [L2191]; generic targets MUST be monomorphized [L2195, L2544].

### 3.15 Field (§3.8.1)

- `{ Modifier } Name [ ":" Type ] [ "=" Expression | Block ]` [L3102]; modifiers precede the name
  [L3116].
- A field MUST appear **only** as a class or structure member [L3117, L3157-3159]; a type body MUST NOT
  define variables or heap identifiers instead [L3118].
- At least one of `:` or `=` MUST appear [L3119]; §2.5 shorthand is allowed [L3120].
- Fields are **not required** to be initialized [L3124]; the initializer's type MUST be the field type or
  implicitly convertible to it [L3125]; with `=`, the field MUST be assigned the expression's result
  [L3126]; with `:` omitted, the type MUST be inferred from `=` [L3127].
- A field MUST enter the instance layout [L3131].
- **Function field**: a field of function type [L3135]; MAY use function shorthand, with `Block` as its
  body [L3136]; `Block` present ⇒ field type MUST be a function type [L3137]; calls MUST go through a
  member access [L3138]; it MUST NOT be judged a method [L3139].

### 3.16 Accessor (§3.8.2)

- `{ Modifier } "accessor" Name ":" Type [ "=>" Backing ] [ AccessorMembers ]` [L3181]; members separated
  by `;` with an optional trailing `;` [L3183]; member forms: value form, `get` block form, `set` block
  form [L3185-3187]; `Backing = Identifier` [L3189].
- `get` and `set` are **convention names, not keywords** [L3203-3204].
- Modifiers precede `accessor` [L3208]; the target type MUST appear [L3209]; no generic parameters
  [L3210, L3409-3411].
- Accessors MAY appear in extension blocks, interface bodies, or scopes outside type bodies and extension
  blocks [L3211]; MUST NOT appear directly in a class or structure type body [L3212, L3213]; at least one
  of `=>` / `AccessorMembers` MUST appear [L3213]; §2.5 shorthand allowed [L3214].
- `get` = read member: parameter list MUST be empty (plus implicit/explicit `this` and, for index
  accessors, index parameters) and return type MUST be the target type [L3219, L3222]. `set` = write
  member: parameters MUST be `value: <target type>` (+ `this`, + index parameters) and return type MUST
  be empty [L3221-3223].
- Non-`static` accessor members inside an extension block MUST explicitly write `this` [L3224];
  accessors outside type bodies/extensions: members MUST NOT carry `this` [L3225].
- Only `get` or only `set` is allowed [L3226]; a member's explicit type annotation MUST match the derived
  result, else a type error [L3227].
- Accessor members MUST NOT be accessed via a member access expression [L3228]; access MUST go through the
  accessor name [L3229].
- Members MUST support only accessibility modifiers [L3230]; member accessibility is relative to the
  accessor's scope [L3231], defaults to the accessor's accessibility [L3232], and MUST NOT exceed it
  [L3233].
- In an extension block: members are **methods** [L3234]. In an interface body: members MUST NOT write
  `this`, and the `this` type MUST be the interface type [L3235]. Outside type bodies/extensions: members
  are **function constants** [L3236].
- **Block shorthand expansions**: `get ( ) B` ⇒ `get = ( ) <T> => B` [L3240]; `get ( this ":" "&" T ) B`
  ⇒ `get = ( this: &T ) <T> => B` [L3241]; `set ( value ) B` ⇒ `set = ( value: <T> ) => B` [L3242];
  `set ( this ":" "&" T "," value ) B` ⇒ `set = ( this: &T, value: <T> ) => B` [L3243].
- `get`'s Block MUST return the target type [L3244]; `set`'s Block MUST NOT return a non-empty value
  [L3245]; with `this`, its type MUST be `&<target type>` [L3246], the accessor MUST be in an extension
  block and MUST NOT be `static` [L3247]; without `this`, the accessor MUST be outside type
  bodies/extensions **or** `static` [L3248]; shorthand is forbidden for index accessors [L3249] and for
  interface-body accessors [L3250].
- **Backing Field**: MUST resolve to an accessible field, variable or constant in the current scope
  [L3254]; its type MUST equal the target type [L3255]; a constant backing ⇒ read-only, no `set`
  [L3256, L3405-3407]; a local-variable backing ⇒ members access the storage **by reference**, and the
  accessor MUST NOT escape the local's scope [L3257-3258]; `set` present ⇒ backing MUST be writable
  [L3259]; in an extension block it is resolved by member-access accessibility rules [L3260]; outside, it
  is resolved from the current scope [L3261].
- **Default implementations**: with `Backing`, get defaults to reading it and set to writing it, and
  unlisted members get these defaults [L3265-3268]; without `Backing`, every member MUST provide an
  implementation expression, which is computational and touches no implicit storage [L3269-3271].
- **Interface**: accessors MUST NOT carry `Backing` [L3275]; members MUST NOT provide implementation
  expressions [L3276]; an implementing class MAY provide a `Backing` implementation in a type-body
  extension [L3277].
- **Type-level accessor**: MAY be `static` [L3281]; members MUST be function constants without `this`
  [L3282]; `Backing` MUST be a same-type `static` field, a global variable, or a global constant [L3283].

### 3.17 Extension (§3.8.3)

- `"extension" [ "for" ExtensionTarget ] Block` [L3419]; target is `Type`, `GenericTarget`, or
  `ArrayTarget` [L3421-3423].
- Inside a type body: target MUST be that type and `for` MUST NOT appear [L3446]; outside: the target
  MUST be specified by `for` [L3447]; the target MUST be one of the three forms [L3448].
- `GenericTarget` MUST start with an identifier; type arguments MUST be written as generic parameters
  [L3449].
- `ArrayTarget` MUST start with `array`; parameters MUST be `name: type`; type arguments MUST be written
  as generic parameters [L3450]; binding names MAY be used in the body [L3451]; for an array target they
  MUST be compile-time constants of cardinal type [L3452].
- Target generic parameters MAY carry constraints; a constrained extension applies **only** to instances
  satisfying the constraint [L3453-3454]; unbound identifiers in type arguments MUST be interpreted as
  generic parameters [L3455].
- Extension members MUST NOT declare fields [L3459, L3525-3530]; MAY use an existing field, variable or
  constant as a Backing Field [L3460]; other rules are identical to in-type members [L3461].
- Members MUST be regarded as members of the target type, but their `private` members MUST be visible
  only inside the extension block [L3462].
- A name collision with an existing member or another extension's member MUST be an error
  [L3463, L3532-3537].
- **Interfaces**: only extensions inside a class type body MAY implement interfaces from that class's
  base list [L3467]; implementing members MUST be in the class body [L3468]; external extensions MUST
  NOT implement interfaces [L3469]; multiple blocks are allowed and their members MUST jointly satisfy
  all interface members [L3470].
- **Visibility**: extensions outside a type body MUST become visible only after `use` [L3474]; `private`
  members are visible only in the extension block [L3475].

### 3.18 Name-elision sugar comparison

| Property | Enumeration [L2678-2699] | Variant [L2824-2847] | Union [L2973-2995] |
| --- | --- | --- | --- |
| Short form | `Type.Name`→`Name` | `Type.Name`→`Name`; `Type.Name(E)`→`Name(E)` | `Type.Name(E)`→`Name(E)` |
| Context count | 10 | 11 (adds `is` left operand) | 10 |
| Blocked by same-name binding | yes | yes | yes |
| Blocked by same-name function | not stated | yes | yes |
| `this` same-name member | must write full form | must write full form | must write full form |
| Resolves to | member access | member access / member construction | member construction only |
| Short-form reads | allowed where context gives type | allowed where context gives type | forbidden; must read via instance member access |
| `match` pattern | target type = matched expr type | target type = match target | target type = matched expr type |
| Full form | always allowed | always allowed | always allowed |

### 3.19 Modifier matrix for §3 declarations

| Declaration | Allowed modifiers (per §3) | Source |
| --- | --- | --- |
| `class` | accessibility, `sealed`, `abstract`, `static` | L2103-2107 |
| class member | accessibility, `virtual`/`override` (methods only), `static`, member-kind modifiers | L2116-2121 |
| `structure` | accessibility, `aligned(N)` | L2332-2334 |
| structure member | accessibility, `aligned(N)`, `static`, member-kind modifiers; no `virtual` | L2344-2349 |
| `interface` | accessibility only | L2494 |
| interface member | accessibility only; `public` mandatory; no `static`; operator contracts implicit `public` with no modifier | L2506-2509 |
| `enumeration` | accessibility only | L2654-2655 |
| enum member | none (no modifier slot) | L2638 |
| `variant` | accessibility only | L2783-2784 |
| variant state | none (no modifier slot) | L2769 |
| `union` | accessibility only | L2936-2937 |
| union member | `aligned(N)` only | L2944 |
| `accessor` | accessibility, `static` (type-level) | L3208, L3281 |
| accessor member | accessibility only | L3230 |
| `extension` | none (no modifier slot) | L3419 |

---

## 4. Exact spellings introduced/used in this range

**Declaration kinds** (lowercase): `class` L2087, `structure` L2318, `interface` L2478, `enumeration`
L2636, `variant` L2767, `union` L2920, `accessor` L3181, `extension` L3419.

**Basic types** (lowercase): `integer` L1518, `cardinal` L1519, `float` L1520, `boolean` L1521.

**Type constructors** (lowercase): `array` L1761, `pointer` L1865, `function` L1918.

**Modifiers / markers**: `public` L2507, `private` L3299, `protected` L2187, `sealed` L2105,
`abstract` L2106, `static` L2107, `virtual` L2118, `override` L2119, `aligned` L2334, `wildcard` L1920
(keyword, L1933).

**Non-keyword names**: `get` and `set` L3185-3187, L3203-3204 (explicitly not keywords); `Backing Field`
L3197 (English term used verbatim).

**Built-in identifiers**: `Object` L2225-2229, `CallingConvention` L1956-1962,
`CallingConvention.PlatformDefault` L1958-1959, `Construct` L2145-2146/L2170, `Destruct` L2153/L2175,
`Type.SizeOf<T>()` L1892, `Cast<T>` L2688, `DivideByZeroException` L1572.

**Calling-convention enum members** (case-sensitive, XREF L10168-10187): `PlatformDefault`,
`CDeclaration`, `StandardCall`, `FastCall`, `SystemVAMD64`, `ThisCall`, `VectorCall`, `MicrosoftX64`.

**Punctuation/operators**: `(` `)` groups L1518-1521/L1918; the `array(` terminal includes the paren
L1761; `:` starts a base list L3048 and separates type/value in fields L3102, enum underlying type
L2636, variant payload L2769, union member L2922, accessor target L3181, `TargetBinding` L3429; `,`
separates base-list targets L3048, array dimensions L1761, `wildcard` L1920; `;` separates calling
conventions L1918/L1949, terminates enum/variant/union members L2638/L2769/L2922, separates accessor
members L3183; `=` enum value L2638 / field init L3102 / accessor impl L3185; `=>` accessor Backing
L3181 and shorthand targets L3240-3243; `#` compile-time-constant parameter convention — participates in
function type equality L1975/L1980, written `#Type` or `#name: Type` L2019/L2022; `&` by-reference
parameter marker — NOT part of the type, does not participate in equality L1979/L2013/L2016, written
`&integer(4)` or `value: &integer(4)`; `@base` escape L2185; `<` `>` monomorphization/generic brackets
L1501/L3048/L3425; `*` appears only in an illegal dereference example L1709 and is not syntax in this
range.

**Casing traps**: `Cast` (capital C) L1609; `Construct`/`Destruct` capitalized L2145/L2153; `Object`
capitalized L2225; `CallingConvention` PascalCase L1960; all other §3 syntax is lowercase.

---

## 5. Example inventory (16 `novasharp` blocks in range)

| Lines | Section | Demonstrates (one line) |
| --- | --- | --- |
| 1487–1506 | §3.1 | Basic/constructed/nested/function/generic-arg types; generic parameters in a type are illegal. |
| 1591–1634 | §3.2 | Basic declarations, `Cast<float(8)>`/`Cast<boolean>`, illegal omitted parens, N=3/16, implicit casts. |
| 1692–1711 | §3.5 | Two references alias one instance; write visible through both; explicit deref illegal. |
| 1835–1858 | §3.6.1 | Fixed arrays, `array(3, 4)` nesting, shorthand + parameter expansion, array↔pointer casts, negative size illegal. |
| 1897–1911 | §3.6.2 | `pointer integer(4)`, target-less `pointer`, pointer cast, deref of target-less pointer illegal. |
| 1999–2078 | §3.6.3 | Function types: unnamed/named/`&`/`#`/`this` params, returns, defaults, `wildcard`, calling conventions, casts, name preservation, illegal forms. |
| 2233–2311 | §3.7.1 | Abstract `Animal`, `Cat` override + `base.Speak()`, sealed `Ragdoll`, implicit `Object`, dispatch; illegal sealed inheritance / missing generic args / bad override / variable in class. |
| 2412–2471 | §3.7.2 | `Vector2D`, `aligned(0)` compact, `CEFHeader` mixed alignment, generic `Box<T>`, struct method with `this: &Counter`; illegal inheritance / `virtual`. |
| 2556–2629 | §3.7.3 | Generic `IValue<T>`, `IArray<T>: IValue<pointer T>`, `Box` implementation, interface-typed local, empty interface; illegal instantiation / struct impl / field / `static` / initialized constant / outside-extension impl. |
| 2703–2760 | §3.7.4 | `Status` enum + extension, elided `Running`, implicit/explicit conversions; illegal duplicate value / omitted underlying / omitted value / generics. |
| 2851–2913 | §3.7.5 | `Status` variant, generic `Outcome<TError>`, payload construction/assignment/comparison; illegal undetermined state / unknown state / wrong payload type. |
| 2999–3041 | §3.7.6 | `Pixel` union, `AlignedBuffer` offset, union inside struct; illegal initializer / non-`aligned` modifier. |
| 3069–3093 | §3.7.7 | Class inheriting one class + one interface; interface inheriting interface. |
| 3143–3174 | §3.8.1 | Uninitialized function field, initialized field, function field body; illegal field outside class/struct, interface field, union field. |
| 3287–3412 | §3.8.2 | Backing-field accessors, `private set`, constant backing, computed accessor, free `get()` accessor, extension `get`/`set` shorthand, free field accessor, `static` accessor; illegal no-backing/no-member, computed without impl, const backing + `set`, generic accessor. |
| 3479–3538 | §3.8.3 | In-body extension, external `extension for Vector3`, array target, generic target, constrained target; illegal field in extension, name collision. |

---

## 6. Open questions, ambiguities, inconsistencies

Each item cites conflicting lines and a proposed reading. Not normative.

**OQ-1 — Interface type kind is never stated.** §3.3/§3.4 classify value/reference; §3.7.1 says class is
reference; §3.7.3 never classifies interfaces [L2475-2552], yet interface-typed locals exist
(`local value: IValue<integer(4)> = Box();` [L2588]) and interface members have no runtime entity
[L2500]. *Proposed*: interface type = reference type; its value is a pointer to an implementing class
instance.

**OQ-2 — `ConstructTargetType = BasicType | Type` is circular/redundant** [L1463, L1718-1725]. `Type`
already includes `BasicType`, and `Type`→`ConstructedType`→`ConstructTargetType`. *Proposed*: treat it
as an alias for "any type", parsed greedily/right-associatively.

**OQ-3 — `Type` alternatives overlap (`Identifier` vs `MonomorphizationExpression`)** [L1463, L1501].
*Proposed*: parse an identifier, and if the next token is `<` in type context continue as a
monomorphization expression; then require the identifier to be generic (§10.2).

**OQ-4 — "Eight binary operators" is never enumerated** [L1558-1565]. *Proposed*: the eight are the
binary entries of §4.9.2's table: `+`, `-`, `*`, `/`, `%`, `==`, `>`, `<` [XREF L4592-4601]; `!=`,
`<=`, `>=` are derived [XREF L4621-4631]; bitwise/logical/shift cannot be overloaded [XREF L4616-4618].

**OQ-5 — "Implicitly convertible to cardinal" for array sizes contradicts "no implicit basic
conversions"** [L1767 vs L1583]. *Proposed*: literals/constant expressions are typed cardinal in size
context; the clause covers user `implicitable` conversions and enum→underlying [L2676], not
integer→cardinal.

**OQ-6 — Signed range of `integer(N)` is unspecified** (only wrap/truncation/remainder sign are given)
[L1569-1578]. *Proposed*: two's-complement `-2^(8N-1) .. 2^(8N-1)-1`.

**OQ-7 — "Array size is compile-time metadata and MUST be stripped" vs VLAs** [L1776-1777 vs
L1787-1793]. *Proposed*: applies only to fixed arrays; a VLA's length is runtime data in the
pointer+length structure.

**OQ-8 — VLA length type is unspecified** [L1790-1792; size expression only constrained to
cardinal-convertible, L1767]. *Proposed*: `cardinal(8)`.

**OQ-9 — Array-parameter expansion naming/order; the example uses syntax §2.5 does not define.**
L1825-1828 says `array(Name: Type) T` ≡ params `Name: Type` + `pointer T`, pointer first; the example
`fixed LogNote(message[length: cardinal(4)]: cardinal(1))` [L1844-1845] expands to
`message: array(length: cardinal(4)) cardinal(1)`, but `ArrayShorthand` in §2.5 only admits `Expression`
dimensions [XREF L409]. *Proposed*: the outer name binds the generated pointer parameter; the binding
name becomes the length parameter; §2.5 must admit `Name ":" Type` dimensions.

**OQ-10 — "VLA cannot be a type-comparison target" vs array equality by size** [L1793 vs L1799-1803].
*Proposed*: VLA comparisons use element type only; VLA↔VLA and VLA→fixed conversions are allowed
[L1816-1817] but unchecked (consistent with L1818's warning).

**OQ-11 — `sealed` methods referenced but absent from §3.7.1's member list** [L2201 vs L2116-2121; §9.11
L8545-8559 allows `sealed` on overriding methods]. *Proposed*: `sealed` is legal on an override; the
§3.7.1 list is incomplete.

**OQ-12 — Enum/variant members have no accessibility syntax** [L2638, L2769], yet are accessed
externally (`Status.Running`) and interface/class members must be `public`. Union members do have a
modifier slot but only `aligned(N)` is allowed [L2922, L2944]. *Proposed*: enum values and variant
states are implicitly public / take the type's accessibility; the union asymmetry is intentional.

**OQ-13 — Variant alignment wording "按各自大小对齐"** [L2807] read literally would align a 9-byte
payload space to 9 bytes. *Proposed*: tag aligned to its size; payload aligned to the max alignment of
its member types; `payload_offset = round_up(tag_size, payload_align)`.

**OQ-14 — Zero-state variants / empty payload space** [L2767 permits an empty body; L2803-2804
undefined for zero states]. *Proposed*: reject an empty variant, or give it size 0/alignment 1 and forbid
construction.

**OQ-15 — Duplicate member names only banned for extensions** [L3463; nothing for variant states, union
members or fields]. *Proposed*: duplicates in any type body are errors (as for enum values L2666).

**OQ-16 — Variant assignment: in-place transition vs value copy** [L1646 vs L2812-2814]. *Proposed*:
lower as an in-place transition (destroy old payload, store new tag/payload), never build a temporary and
overwrite; mirrors §4.7 [XREF L4141-4142].

**OQ-17 — Union construction "from the memory start point" vs member offsets** [§4.7 XREF L4148 vs
L2962-2970]. *Proposed*: §4.7 is loose; construction writes at the constructed member's offset, otherwise
`aligned(8) Payload` would clobber `Header`. L2962-2970 is normative.

**OQ-18 — Union's own alignment is unspecified** [L2965 defines size only]. *Proposed*: alignment = max
member alignment that its offset actually satisfies, else 1; a conservative implementation may use
alignment 1 with byte-wise access.

**OQ-19 — Struct's own alignment and `aligned(0)` tail padding** [L2361-2367]. *Proposed*: struct
alignment = max used field alignment (1 when none/`aligned(0)`); size rounded to it; `aligned(0)` gives
exact sum-of-sizes layout.

**OQ-20 — `aligned(M)` on a field smaller than natural alignment** [L2364]. *Proposed*: allowed;
unaligned loads/stores or memcpy; `M = 0` means no requirement (offset unchanged).

**OQ-21 — `aligned(N)` on class fields** (excluded by §3.7.1's list [L2116-2121] and §9.14's target list
[XREF L8621-8624]). *Proposed*: reject; class layout is unspecified anyway (OQ-22).

**OQ-22 — Class layout is entirely unspecified beyond "type info + vtable"** [L2124-2131: no header
size/position, base-subobject position, field order, vtable slot order, interface tables]. *Proposed*:
header `{vtable, typeinfo}` at offset 0; base subobject next; fields in declaration order; virtual slots
base-first then new virtuals; interfaces via per-class interface tables referenced from typeinfo. Needs a
dedicated spec section.

**OQ-23 — Interface conversions "reinterpretation" vs §14.3 requiring runtime type info** [L2548 vs
XREF L9830]. *Proposed*: pointer bitcast **plus** a runtime check that the class implements the
interface (no content change); failure behavior is unspecified — use `trap` or define an exception.

**OQ-24 — Empty type as a variant payload is unspellable** [L1750 allows it; L1747 says no written form;
L2769 requires a `Type` after `:`; omitting `:` means no payload L2793]. *Proposed*: either forbid it or
define `Name: ;` / `Name: function()` as an empty payload. Real grammar gap.

**OQ-25 — Static constructor is called a "常量" (L2146, §9.15 L8681-8684) but constructors are "方法"
(L2161)** and its example has no `this` [XREF L8740]. *Proposed*: a static constructor is a
`static fixed` function constant in an extension block, no `this`, run before `Main`.

**OQ-26 — Class destructor order across the inheritance chain is unspecified** (constructors are
explicitly top-down, L2137; destruction only says "call the destructor", L2139-2141). *Proposed*:
most-derived to base, then reclaim.

**OQ-27 — `Object.base` self-reference** [L2227]. *Proposed*: inside `Object`, `base` aliases the same
instance; base-member lookup in `Object` finds only `Object`'s own members (no recursion).

**OQ-28 — Empty accessor member list and `get;` without Backing** [L3183, L3213, L3269-3270].
*Proposed*: `{ }` does not satisfy "at least one of `=>`/members"; `get;`/`set;` without `Backing` is an
error (only valid in interface bodies or with `Backing`).

**OQ-29 — `get`'s "parameter list MUST be empty" vs explicit `this` and index parameters** [L3219 vs
L3222, L3224, L3234, L3241]. *Proposed*: "empty" excludes only ordinary parameters; `get` = `[this]` +
index parameters; `set` = `[this]` + index parameters + `value`.

**OQ-30 — Accessor shorthand requires `this: &目标类型`, but class methods use `this: ClassName`**
[L3246 vs L2239, L3368; ref-to-ref forbidden L1680-1682]. *Proposed*: `&T` is a by-reference passing mode,
not a type; for a class it means a pointer to the reference slot (double indirection), for a struct a
pointer to the struct; both lower to `ptr`. `&` is excluded from type equality [L1979].

**OQ-31 — Extension target grammar cannot express a concrete array target** (`ArrayTarget` requires a
trailing `GenericParameterDefinition` [L3427], so `extension for array(3) integer(4)` only parses as a
`Type`, without bindings). *Proposed*: lookahead `array` + `(` + `Name ":"` ⇒ ArrayTarget, else plain
type.

**OQ-32 — `GenericTarget` puts parameter *definitions* in type-argument position** [L3425, L3455].
*Proposed*: this deliberately introduces fresh generic parameters (not a monomorphization expression);
the parser must distinguish it from `BaseList` monomorphization.

**OQ-33 — `wildcard` admitted in `ParameterList` but not in function-literal `LiteralParameters`**
[XREF L7501; §2.5 shorthand allows it XREF L407; L1829 applies array expansion to literals]. *Proposed*:
`wildcard` is allowed in function types and `fixed` shorthand declarations, not in function literal
parameter lists.

**OQ-34 — Default values excluded from function-type identity** [L1979, L2010]. *Proposed*: types
differing only in defaults are identical; the destination type's defaults govern the call site. Note the
hazard: a `Cast` can silently change call-site defaults.

**OQ-35 — `&` excluded from equality means by-ref and by-value function types are identical** [L1979;
XREF L7549-7551]. *Proposed*: keep the rule, but treat casts that change `&`-ness as an ABI hazard
(pointer vs value lowering); reject or warn.

**OQ-36 — "No written form" for the empty type vs `function()`/`pointer`** [L1747, L1967-1969,
L1991-1993, L1745]. *Proposed*: the empty type is denoted by **omission** inside a constructor; a bare
`function` (no parens) should be rejected despite the optional EBNF group, because it would omit
parentheses for an omitted return type (L1969) and L1993's sugar requires the return type to be present.

**OQ-37 — Enum elision contexts exclude `is` while variant contexts include it** [L2681-2691 vs L2836].
*Proposed*: enum `is` requires the full form; variant `is` allows the short form. Intentional asymmetry
worth documenting.

**OQ-38 — Enum underlying type may be `boolean` or `float(N)`** (`":" BasicType` [L2636]; only "must be
a basic type" [L2648]). *Proposed*: reject `boolean`/`float(N)` underlying types as unsound, or define
their semantics; the grammar currently permits them.

**OQ-39 — Target-less pointer and `SizeOf<T>()`** [L1750, L1885, L1892-1893]. *Proposed*: `pointer` with
an empty target is exactly the 1-byte-arithmetic case; no other empty-target spelling exists.

**OQ-40 — Free accessors outside type bodies lack scoping/visibility rules** [L3211, L3236, L3261].
*Proposed*: module-scope constant-like members obeying §6.1, with `Backing` resolved in the enclosing
scope.

**OQ-41 — `this` parameter type for class constructors: `ClassName` vs `&ClassName`** [L2170 vs L2444-
2451]. *Proposed*: class ⇒ `this: ClassName` (reference); struct ⇒ `this: &StructName`; both lower to
`ptr`; accessor `&<target type>` follows OQ-30.

**OQ-42 — Interface constants "must not be initialized"** [L2502] conflicts with the general
"constants must be initialized" rule [XREF L5690-5691]. *Proposed*: interface constants are pure
contracts; implementation provides the value.

**OQ-43 — Parameter names in type identity vs preservation** [L1979 vs L1981]. *Proposed*: canonical
identity drops names; diagnostics/printing keep them; types differing only in names are interchangeable.

**OQ-44 — `array(length: cardinal(4)) T` is not derivable from `ArrayTypeConstructor`** [L1761 vs
L3511]. *Proposed*: `Name ":"` after `array(` is the ArrayTarget binding production, reachable only via
that context or the parameter-expansion sugar.

---

## 7. LLVM 18 codegen implications

Notation: `iN` = LLVM integer; all pointers are opaque `ptr` (LLVM 18); `GEP` = `getelementptr`;
`align` = LLVM alignment attribute.

### 7.1 Type mapping

| NovaSharp | LLVM 18 | Notes |
| --- | --- | --- |
| `integer(1/2/4/8/16)` | `i8/i16/i32/i64/i128` | signed ops |
| `cardinal(1/2/4/8/16)` | `i8/i16/i32/i64/i128` | unsigned ops (`udiv`, `urem`, `lshr`, `uitofp`) |
| `float(4)` / `float(8)` | `float` / `double` | IEEE binary32/64 |
| `boolean` | `i8` | MUST be 1 byte [L1554]; canonical 0/1, normalize on store |
| empty type | `void` return / `ptr` target | no layout [L1746-1750] |
| `pointer T`, `pointer` | `ptr` | target type stays front-end only |
| `array(N) T` | `[N x T]` | inline, contiguous |
| VLA | `{ ptr, i64 }` | pointer first, length second [L1790-1792] |
| `function(...)` | `ptr` code pointer (+ env when capturing) | reference type [L1915] |
| class `C` | `ptr` to instance body | header + fields |
| `structure S` | by-value aggregate | exact spec layout, no header |
| `enumeration E: B` | `B` | no wrapper [L2670-2671] |
| `variant V` | `{ tag, [payload_size x i8] }` | tag first [L2806] |
| `union U` | `[size x i8]` | offsets 0 or N [L2963-2965] |

Unsupported N MUST be emulated [L1530, L1534, L1537]: `i128` on 32-bit lowers to libcalls; soft-float
for missing FPUs; never widen `float` operations (L1550).

### 7.2 Basic-type operations

- `+`/`-`/`*`: plain `add`/`sub`/`mul`, **never** `nsw`/`nuw` (wrap, L1569); unary negation
  `sub iN 0, x` (L1577).
- `sdiv`/`srem` (integer) and `udiv`/`urem` (cardinal) match truncation (L1570) and remainder sign
  (L1571).
- Divisor `0`: LLVM `div`/`rem` by zero is poison — emit an explicit compare/branch to a
  `DivideByZeroException` throw helper (L1572). Also special-case `INT_MIN / -1` (poison in LLVM) to
  wrap to `INT_MIN` and remainder 0 (L1569).
- Shifts: LLVM shift counts ≥ bit width are poison; mask the count with `N-1` before `shl`/`lshr` to
  implement "count mod bit width" (L1573). Left shift uses `shl`, right shift uses `lshr` (never `ashr`), matching "逻辑左移/右移".
- `sitofp`/`uitofp` for int→float; do not enable `fast` flags and do not widen intermediates
  (L1550).
- Float→int MUST wrap (L1585), but `fptosi`/`fptoui` yield poison out of range: check range, reduce
  modulo 2^N with `frem` if needed, convert, truncate. NaN behavior is undefined by L1585 — choose a
  diagnostic or `0`.
- Boolean: to bool = `icmp ne x, 0` + `zext i1 to i8`; from bool = `zext` after normalizing (L1586-1587).
- Float: `fadd/fsub/fmul/fdiv` with no fast-math (round-half-to-even L1574, NaN L1575-1576); division by
  zero already yields inf/NaN, no exception (L1576).
- Float compares: `==` ⇒ `oeq`; `!=` ⇒ `une`; `<` ⇒ `olt`; `>` ⇒ `ogt`; `<=` ⇒ `ole`; `>=` ⇒ `oge`
  (ordered forms are false for NaN, matching L1575 and §4.9.2).

### 7.3 Value vs reference

- Value-type identifiers (arrays, pointers, structs, enums, variants, unions): inline `alloca`/global;
  assignment = aggregate `load`+`store` or `llvm.memcpy` (body copy, L1646).
- Reference-type identifiers (class, function): `ptr`; assignment = `store ptr` (reference copy, L1659).
- Class values are pointers to the instance body, never to a reference slot (L1678). Member access =
  load instance pointer + `GEP` (implicit deref); there is no `*` operator (L1687).
- Aliasing is expected (L1674): never add `noalias` to class-reference parameters.
- `&` parameter = `ptr` to the argument storage [XREF L7549-7551]; class + `&` = pointer to the
  reference slot (OQ-30); struct + `&` = pointer to the struct.

### 7.4 Arrays and VLAs

- `array(N) T` → `[N x T]`, inline; multi-dimensional `array(X, Y) T` → `[X x [Y x T]]` (L1822-1824).
- array→pointer: `getelementptr [N x T], ptr %a, i64 0, i64 0` (L1807-1809). pointer→array: no-op bitcast
  with a Warning and no size check (L1818).
- VLA local: `{ ptr, iN }` (length type `i64`, OQ-8), pointer first (L1790-1792); allocate the buffer
  with the instance's storage duration, store pointer then length; VLAs are non-returnable (L1789); no
  runtime size metadata in artifacts (L1776-1777).

### 7.5 Pointers

- Pointer casts are no-ops (or `bitcast`) and MUST NOT change the address (L1880); warn (L1881).
- `p ± n` → `getelementptr T, ptr %p, iN %n` (unit `SizeOf<T>()`, L1892); target-less → `getelementptr
  i8` (unit 1 byte, L1893).
- `p − q` → `ptrtoint` both, subtract, `sdiv` by `SizeOf<T>()` (element count, L1891).
- Target-less pointer deref is a front-end error (L1885); the spec defines no null value for `pointer`.

### 7.6 Function types, calling conventions, `#`, `wildcard`, closures

- Function values are references (L1915): a variable holds `ptr`; assignment copies it (L1659).
  Captured function values share code and keep environment data [XREF L9836-9840], so capturing
  function values need a fat value `{ ptr code, ptr env }` (or a hidden env argument); uncaptured ones
  may be plain function pointers. §3.6.3 gives no layout, so this is a codegen decision; the front end
  must know whether a function type's values can capture (§2.7.9 captures [XREF L1076-1080]).
- Explicit function-type casts are unrestricted (L1986-1987) and only change call behavior (L1985):
  lower as a no-op bitcast + Warning; do not insert adapters (a mismatched call is UB by the asserted
  signature).
- Calling conventions: `PlatformDefault` → target default (unannotated `call`); `CDeclaration` → `ccc`;
  `StandardCall` → `x86_stdcallcc`; `FastCall` → `x86_fastcallcc`; `SystemVAMD64` → `x86_64_sysvcc`;
  `ThisCall` → `x86_thiscallcc`; `VectorCall` → `x86_vectorcallcc`; `MicrosoftX64` → `win64cc`
  [XREF L10168-10187]; reject/emulate unsupported targets (L1955-1959).
- `#` parameters participate in identity and overload resolution (L1975-1980) but have **no runtime
  argument**: specialize on their values and erase them from the ABI signature.
- `wildcard` has no runtime representation in the type (L1932-1944). If a declaration defines a variadic
  function with it, lower as LLVM varargs (`...`) with C promotions; if it appears only in a function
  type used for casts, lower as a plain function pointer. The ABI is not specified (OQ-33).
- `this` is the first explicit parameter and lowers to `ptr` (class: reference; struct: `&T`).

### 7.7 Classes: layout, vtables, interfaces, RTTI

Minimum spec: instances MUST carry type info + vtable (L2130); static/methods/accessors excluded
(L2126-2129); `Object` is the implicit base (L2226). Everything else is unspecified (OQ-22).
Recommended:

```
%Class = type { ptr /*vtable*/, ptr /*typeinfo*/, <base fields...>, <own fields...> }
```

- Base subobject immediately after the 2-word header; fields in declaration order; no reordering.
- vtable `{ ptr typeinfo, ptr slot0, ... }`: base virtual slots first in base declaration order, then new
  virtuals; `override` replaces the slot (L2199-2209); non-virtual methods get no slot (L2215).
- `base.M(...)` = direct call to the base implementation, never through the vtable (L2214); no
  non-virtual slot needed.
- Virtual call: load vtable from the header, load the slot, indirect call (L2213).
- Interfaces produce no runtime entity (L2500), so an interface value is a `ptr` (OQ-1). Per-class
  interface table: assign each interface (including each monomorphization) a stable global id; typeinfo =
  `{ ptr name, ptr base_typeinfo, ptr iface_table, i32 iface_count }`; iface_table = sorted
  `{ i32 id, ptr itable }`; each itable holds interface method slots in interface declaration order.
  Interface call: look up the id (dense array for small counts, binary search otherwise), load the slot,
  indirect call.
- Interface→class conversion is explicit (L2549) and requires a runtime membership check (OQ-23);
  class→interface implicit (L2550) may skip it when the static type guarantees implementation.
- Covariant `this` in overrides/implementations (L2204, L2524) is erased at the ABI (all `this` are
  `ptr`); it is a front-end check only.
- RTTI: needed for `is`, interface conversions and `trap` [XREF L9830]; metadata minimization MAY strip
  unused slots and merge identical descriptors [XREF L9829-9834], but generic instances MUST keep
  distinct identity [XREF L9826].
- No default allocation (L2084): `local`/`heap`/`manual heap` choose storage [XREF L9408-9430]; a class
  instance may live on the stack; escape analysis enforces scope rules [XREF L9463-9469].

### 7.8 Structures: exact layout algorithm

Do **not** rely on LLVM's ABI layout: the spec's default field alignment is "the member type's size"
(L2363), which may differ from target ABI alignment (e.g. an 8-byte member on a 32-bit target).

1. `offset = 0`, `max_align = 1`.
2. For each field in declaration order (L2359): `a = aligned(M)` if present, else `N` if the struct
   declared `aligned(N)`, else `sizeof(field type)` (L2361-2364). If `a > 0`: `offset =
   round_up(offset, a)`, `max_align = max(max_align, a)`. If `a == 0`: no padding, `max_align`
   unchanged (L2365). Assign the offset; `offset += sizeof(field type)`.
3. Size = `round_up(offset, max_align)` using only non-zero alignments (L2366-2367); with `aligned(0)`
   the size is the exact sum of field sizes.
4. Empty struct: size 0, alignment 1.

Lowering: emit a **packed** struct `<{ ... }>` with explicit `[pad x i8]` padding fields, `align
max_align` on globals/alloca, and field access via `GEP` at computed offsets; or use a normal struct only
when spec layout coincides with the ABI. For `aligned(M)` smaller than natural (OQ-20), use `align 1`
loads/stores or memcpy. Inline arrays are `[N x T]` (L2368-2369). Structs have no vtable/RTTI header
(L2358). Copies copy padding too (padding is value-less).

### 7.9 Enumerations

- Lower as the underlying `iN` with no wrapper (L2670-2671); members are constants with unique values
  (L2666); casts are bitcasts/truncations; enum→underlying is a no-op (L2675-2676); name elision is
  resolved in the front end (L2680-2699).

### 7.10 Variants

- tag type = smallest `i8/i16/i32/i64/i128` representing `state_count` (L2803-2804).
- payload space = union of payload types (L2805): `payload_size = max sizeof`, `payload_align = max
  alignof`; `payload_offset = round_up(sizeof(tag), payload_align)`; tag first (L2806); total size =
  `round_up(payload_offset + payload_size, max(alignof(tag), payload_align))`.
- No-payload variants: `payload_size = 0` (still a defined size/alignment); zero-state variants: reject
  (OQ-14).
- Construct: store tag, store payload at `payload_offset` with payload alignment (or memcpy).
- Equality (L2818-2822): compare tags, then switch on the tag and compare payloads with the payload
  type's equality (recursively; float NaN rules apply).
- Reassign (L2812-2814): branch on the old tag; destroy the old payload if any; store the new tag and
  payload. Never build a temporary and overwrite (OQ-16). `memcpy` is valid only when the old payload has
  no destructor.

### 7.11 Unions

- `%U = type [size x i8]`, size = max(offset + member size) (L2965); offsets are byte-exact `0` or the
  `aligned(N)` value and MUST NOT be adjusted for platform alignment [XREF L8618-8620].
- No tag and no active-member tracking: unconstructed members may be read and written (L2971).
- Member access: `GEP` to the offset, `load`/`store` with `align = min(natural_align, offset_alignment)`
  or `align 1`; use `llvm.memcpy` for aggregate/array members to avoid misalignment UB.
- Construction: write only the constructed member; leave the rest uninitialized — do not zero
  (zeroing would initialize unconstructed members, forbidden by L2955). Reads of uninitialized members
  must follow "uninitialized value" semantics [XREF L9452-9459]; use `freeze` on byte loads feeding
  branches/output to avoid LLVM poison propagation.
- Union alignment: 1 (safe) or max member alignment when offsets permit (OQ-18). Type punning is legal:
  do not attach TBAA that would make it UB, and do not mark member loads with strict alias info.

### 7.12 Fields, accessors, static members

- Field: `GEP` into the aggregate; value type inline; function field = `ptr` slot, call through the slot
  (L3138); function fields are not methods (L3139) — no vtable slot, no implicit `this`.
- Accessors lower to getter/setter functions: extension/interface members are methods — `T @get(ptr
  this)`, `void @set(ptr this, T value)` (L3234, L3219-3221); free accessors are function constants with
  no `this` (L3236); `static` accessors have no `this` (L3282); interface accessors become interface
  table slots implemented by the class extension (L3275-3277).
- Backing: `=> _field` gives default get = load, default set = store (L3265-3268); a constant backing
  generates no setter (L3256); a local-variable backing closes over stack storage (L3257-3258) — lower as
  an env-carrying function (`{ ptr code, ptr env }`) or inline; it must not escape the local's scope.
- Static fields: module-level globals; compile-time-known initializers go to a writable section,
  otherwise the Launcher initializes at runtime [XREF L8679]. Static constructors run before `Main`
  [XREF L8683-8684] via `llvm.global_ctors` or an explicit launcher sequence.

### 7.13 Constructors, destructors, initialization

- Instance constructor: `void @C.Construct(ptr this, ...)` (L2145, L2170); field initialization top-down
  along the inheritance chain (L2137); base constructor call as the first statement (L2147).
- Struct constructor: `void @S.Construct(ptr this, ...)` with `this: &S` (L2444-2451).
- Static constructor: `void @C..cctor()` with no `this` [XREF L8683-8684].
- Destructor: `void @C.Destruct(ptr this)` (L2153); run most-derived→base (OQ-26); a throwing destructor
  MUST terminate and skip subsequent destruction (L2155) — route through `terminate` in the landing pad;
  do not free heap storage if it did not complete (L2156).
- Default ctor/dtor sugar (L2167-2176, L2399-2408): synthesize the empty function; emit only if used.
- Member-init expressions are evaluated during construction, reordered if needed, warning on failure
  (L2384-2385), with the instance's own allocation (L2149, L2386) — no hidden allocation.
- Construction expressions (`Counter()`, `Pixel.RGBA(...)`) construct **directly into the target
  storage**; no intermediate-then-overwrite [XREF L4141-4142]. For `local`, pass an `alloca` as the
  result location (sret-like) and run the constructor on it.

### 7.14 Extensions, monomorphization, metadata

- Extension members are target-type members with mangled symbols; in-body extension members lower as
  ordinary methods; external extensions are free functions with `this` first, visible only after `use`
  (L3474); `private` extension members get internal linkage and are invisible outside the block
  (L3462, L3475).
- Extensions MUST NOT add fields (L3459): no layout change; enforce at type-check time. Name collisions
  with existing or other-extension members are hard errors (L3463).
- Generics: monomorphize every instantiation; each gets a distinct LLVM type and distinct static fields
  [XREF L9826]; `#` values are part of the mangled identity (L1980).
- Array-target extensions (L3427) specialize per element type and dimension bindings; dimension bindings
  are compile-time `cardinal` constants and do not exist at runtime (L3452).
- Metadata minimization (XREF L9829-9834): strip type names, unused vtable slots and unused interface
  entries, but keep what `is`, interface casts and `trap` require. Fixed array sizes MUST NOT become
  runtime metadata (L1776-1777).

---

## 8. Out-of-range dependencies consulted (no invented rules)

| Topic | Section | Lines | Why needed |
| --- | --- | --- | --- |
| Definition shorthand | §2.5 | 400–510 | field/accessor/array member shorthand |
| Keywords | §2.6 | 511–551 | `wildcard`/`this`/`base`/`extension`/`for` |
| Function literal/capture | §2.7.9 | 1076–1080 | function values and environments |
| `Cast<T>` | §4.3 | 3678–3707 | explicit cast spelling, `implicitable` |
| Member construction | §4.7 | 4110–4190 | variant/union construction, direct-into-storage |
| Operator overloads | §4.9.2 | 4588–4655 | the eight binary operators, derived operators |
| Parameters | §8.2 | 7492–7560 | `Parameter` grammar, `#`, `&`, defaults, `this` |
| Modifiers | §9.10–§9.15 | 8527–8720 | `abstract`, `sealed`, `virtual`, `override`, `aligned`, `static` |
| Generic parameters | §10.1 | 8784–8850 | which declarations take generics, constraints |
| Monomorphization | §10.2 | 8916–8965 | `<...>` with `;`-separated args, empty args |
| Memory allocation | §12.1 | 9408–9475 | storage durations, no default allocation, uninitialized storage, escape |
| Optimization | §14.3 | 9826–9845 | type identity, metadata minimization, function values |
| Built-In Library | §15.2 | 10157–10210 | `Object`, `CallingConvention`, exception classes |

---

*End. Scope: spec lines 1456–3539; every normative bullet is line-tagged; uncertain readings are
collected as OQ-1…OQ-44 in §6.*
