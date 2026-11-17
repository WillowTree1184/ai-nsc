# NovaSharp Draft 1.1 — §11–§15 Implementation Note

Compile-time execution · memory model (NOLAA) · ABI + name mangling · program startup · appendices (§15.1/§15.2/§15.3)

- Source: [`docs/grammar/Draft 1.1.md`](../grammar/Draft%201.1.md), 10,425 lines. Range covered: **L8951–L10425**, read in full (no prose skipped).
- Scope of this note: §11 编译期 (11.1–11.8), §12 内存模型 (12.1–12.2), §13 ABI (13.1–13.2), §14 程序 (14.1–14.3.3), §15 附录 (15.1 语法速查表, 15.2 Built-In Library, 15.3 Compile-Time Library).
- Only file written by this task. No implementation code. Nothing invented: every rule below is anchored to spec lines; anything not stated in the spec appears only in the Open questions section.

## 0. Reading conventions

| Symbol | Meaning in the spec | Used below as |
| --- | --- | --- |
| 必须 | mandatory | **MUST** |
| 不得 | prohibited | **MUST NOT** |
| 允许 | permitted | **MAY** |
| 禁止 | forbidden | **FORBIDDEN** |
| 定义 | stipulative definition | **DEFINES** |
| 当 … 时 | conditional premise | "when …" |
| 例如 | non-normative example | "Example" |

Anchors are written `L####` / `L####–L####` and refer to `Draft 1.1.md` line numbers. Identifiers, keywords, function names, type names and file names are kept exactly as spelled in the spec (including `NOLAA`, `linkable`, `manual heap`, `#{}`, `#()`, `#if`, `Type.SizeOf<T>()`, `Compiler.LogNote`, `AllocateFailedException`, `IIterable<T>`, `Result<TValue; TAlternative>`).

## 1. Section map

| § | Title | Lines | Contents | Downstream consumer |
| --- | --- | --- | --- | --- |
| 11 | 编译期 | L8951–L9404 | compile-time execution model | sema, const-eval |
| 11.1 | 编译期执行 | L8953–L8972 | `#` forms, whitespace, `#` not a type prefix, `#[]` not a form | lexer/parser |
| 11.2 | 编译期执行顺序 | L8974–L9047 | whole-program parse, file order, top-level vs body execution, `#if` interleaving, residuals | driver, sema |
| 11.3 | 编译期表达式 | L9049–L9089 | `#(Expression)`, result-type legality | sema, const-eval |
| 11.4 | 编译期块 | L9091–L9156 | `#{ Block }`, no scope, placement, control flow, terminal products, side effects | sema, const-eval |
| 11.5 | 编译期常量参数 | L9158–L9221 | `#Name: Type` / `#Type`, monomorphization, type equality, overloads | parser, sema, monomorphizer |
| 11.6 | 条件编译 | L9223–L9300 | `#if` / `else`, branch semantics, placement, per-instance evaluation | parser, sema |
| 11.7 | 编译期边界 | L9302–L9391 | ct constants, ct variables, ct↔runtime boundary, side effects, allocations, compiler API contract | sema, codegen |
| 11.8 | 编译器 API | L9393–L9404 | ct library extension policy | docs, sema |
| 12 | 内存模型 | L9406–L9619 | allocation & lifetime | sema, MIR, codegen |
| 12.1 | 内存分配 | L9408–L9503 | 3 storage durations, construction, failure, uninitialized storage, escape, release | sema, codegen |
| 12.2 | NOLAA | L9505–L9619 | responsibilities, safe points, insertion, abandon, API, checks, SCC, destructors | NOLAA pass |
| 13 | ABI | L9621–L9675 | binary interface | codegen |
| 13.1 | NovaSharp ABI | L9623–L9636 | composition: calling convention + layout + mangling | codegen, runtime |
| 13.2 | 名称修饰 | L9638–L9675 | `N` + path + name + FNV-1a-64 hash; `linkable` symbol overrides | codegen |
| 14 | 程序 | L9677–L9951 | program-level contract | driver, codegen |
| 14.1 | 主函数 | L9679–L9717 | exactly one `Main`; only Launcher calls it | driver, sema |
| 14.2 | 启动流程 | L9719–L9795 | entry point → Launcher → static ctors/init → `Main` → return value | codegen, runtime |
| 14.3 | 编译优化 | L9797–L9951 | optimization constraints & budgets | LLVM pipeline |
| 14.3.1 | 编译优化总则 | L9799–L9881 | shared constraints, DCE, dedup, metadata, devirt, representation, budgets | pipeline |
| 14.3.2 | 体积优化 | L9883–L9899 | literal pooling; ≥2% / ≥10% net decrease | pipeline |
| 14.3.3 | 速率优化 | L9901–L9951 | trap fast path, inlining, specialization, forbidden opts, pass table | pipeline |
| 15.1 | 语法速查表 | L9955–L10155 | authoritative syntax summary (bullet list, some productions elided with `…`) | parser |
| 15.2 | Built-In Library 完整内容 | L10157–L10359 | 13 mandatory definitions | sema, runtime |
| 15.3 | Compile-Time Library 完整内容 | L10361–L10421 | `Type`, `Compiler`, `NOLAA` modules | sema, const-eval |
| — | License footer | L10423–L10425 | CC-BY-4.0 | — |

Sub-anchor list (for cross-references used by the spec itself): §11.1 `#111-编译期执行`, §11.2 `#112-编译期执行顺序`, §11.3 `#113-编译期表达式`, §11.4 `#114-编译期块`, §11.5 `#115-编译期常量参数`, §11.6 `#116-条件编译`, §11.7 `#117-编译期边界`, §11.8 `#118-编译器-api`, §12.1 `#121-内存分配`, §12.2 `#122-nolaa`, §13.1 `#131-novasharp-abi`, §13.2 `#132-名称修饰`, §14.1 `#141-主函数`, §14.2 `#142-启动流程`, §14.3.1 `#1431-编译优化总则`, §14.3.2 `#1432-体积优化`, §14.3.3 `#1433-速率优化`, §15.2 `#152-built-in-library-完整内容`, §15.3 `#153-compile-time-library-完整内容`.

## 2. Verbatim EBNF blocks in L8951–L10425

The range contains exactly **three** EBNF blocks (fence census: L9035/9045 novasharp, L9053/9055 ebnf, L9083/9089 novasharp, L9095/9097 ebnf, L9145/9156 novasharp, L9201/9215 novasharp, L9219/9221 novasharp, L9227/9231 ebnf, L9280/9300 novasharp, L9377/9391 novasharp, L9482/9503 novasharp, L9589/9619 novasharp, L9695/9717 novasharp, L9778/9795 novasharp, L10161/10359 novasharp, L10367/10421 novasharp). Copied byte-for-byte.

### 2.1 §11.3 编译期表达式 — `CompileTimeExpression` (L9053–L9055)

```ebnf
CompileTimeExpression = "#" "(" Expression ")" .
```

### 2.2 §11.4 编译期块 — `CompileTimeBlock` (L9095–L9097)

```ebnf
CompileTimeBlock = "#" Block .
```

### 2.3 §11.6 条件编译 — `CompileTimeIf`, `CompileTimeBranch` (L9227–L9231)

```ebnf
CompileTimeIf     = "#if" Expression CompileTimeBranch [ "else" CompileTimeBranch ] .

CompileTimeBranch = Block .
```

No other §11–§15 section defines new productions in an EBNF block; §11.5 (compile-time constant parameters) and §13.2 (mangling) are prose-only and are reflected as dotted-item annotations in §15.1 (`Parameter = [ "#" ] …`, `LiteralParameter = [ "#" ] …`).

## 3. §15.1 语法速查表 — consolidated syntax summary

Formatting note: §15.1 (L9955–L10155) is a bullet list of one-line syntax forms, each with a Chinese gloss and a link to the governing section; a bullet may also be a pointer-only entry with no production. Below, every bullet is reproduced in spec order, grouped under §15.1's own category headings, with the production kept **verbatim** (including the spec's `…` truncations) and the gloss translated. Bullets that carry no production are listed with their category so nothing is dropped. `…` marks where the spec itself elides alternatives — see “Completed productions” below, and Q1.

### 3.1 有关词法 (Lexical) — L9959–L9980

Pointer-only entry: reserved words with fixed semantics (§2.6) · L9961.

- `FunctionLiteral = "(" [ LiteralParameters ] [ ";" CallingConventionExpression ] ")" [ Type ] "=>" Block .` — function literal implements the callee's convention; the only way to create a function (§2.7.9) · L9962
- `CharacterLiteral = "'" ( Character | EscapeSequence ) "'" [ ":" CharacterEncodingTag ] .` — maps a character to a cardinal literal (§2.7.4) · L9963
- `StringLiteral = NormalStringLiteral | VerbatimStringLiteral . …` — maps a string to an array of a cardinal type (§2.7.5) · L9964
- `Literal = IntegerLiteral …` — syntax form denoting a constant (§2.7) · L9965
- `BooleanLiteral = "true" | "false" .` — boolean literal (§2.7.3) · L9966
- `ArrayLiteral = "{" [ Element { "," Element } [ "," ] ] "}" . …` — array literal (§2.7.8) · L9967
- `IntegerLiteral = DecimalLiteral | HexLiteral | BinaryLiteral . …` — integer literal (§2.7.1) · L9968
- `InfiniteLiteral = "infinite" .` — IEEE 754 positive infinity (§2.7.10) · L9969
- `UninitializedLiteral = "uninitialized" .` — uninitialized-storage marker (§2.7.12) · L9970
- `FloatLiteral = DecimalLiteral "." DecimalLiteral [ Exponent ] . …` — floating-point literal (§2.7.2) · L9971
- `CharacterEncodingTag = "ascii" …` — character-set tag for characters and strings (§2.7.6) · L9972
- `EscapeSequence = "\" ( "a" | "b" | "f" | "n" | "r" | "t" | "v" | "\" | '"' | "'" | "0" ) …` — escape combination in characters and strings (§2.7.7) · L9973
- `IllegalLiteral = "illegal" .` — IEEE 754 NaN literal (§2.7.11) · L9974
- `Identifier       = Name | MemberAccessExpression . …` — developer-defined name, or a member-access expression (§2.4) · L9975

Pointer-only entries: comments (ignored) §2.3 · L9976; source text files §2.1 · L9977; tokens/whitespace §2.2 · L9978; keyword-to-symbol semantic table §2.9 · L9979; statement terminators §2.8 · L9980.

### 3.2 有关名称 (Names) — L9982–L9987

Pointer-only entries: scope boundaries §6.1 · L9984; accessibility §6.2 · L9985; identifier uniqueness per scope §6.3 · L9986; shadowing §6.4 · L9987. No productions.

### 3.3 有关类型 (Types) — L9989–L10013

- `BasicType = IntegerType …` — built-in base types (§3.2) · L9992
- `FieldDefinition = { Modifier } Name [ ":" Type ] [ "=" Expression | Block ] .` — data member in a type body (§3.8.1) · L9995
- `IdentifierDefinitionShorthand = FunctionShorthand | ArrayShorthand .` — definition using a parameter list or dimension list in place of a type annotation (§2.5) · L9996
- `FunctionShorthand = Name [ GenericParameters ] "(" [ Parameters | "wildcard" ] [ ";" CallingConventionExpression ] ")" [ ":" Type ] .` — function shorthand (§2.5) · L9997
- `ArrayShorthand = Name "[" Expression { "," Expression } "]" [ ":" Type ] .` — array shorthand (§2.5) · L9998
- `ExtensionDeclaration = "extension" [ "for" ExtensionTarget ] Block . …` — declaration appending members to a type (§3.8.3) · L9999
- `AccessorDefinition        = { Modifier } "accessor" Name ":" Type [ "=>" Backing ] [ AccessorMembers ] . …` — identifier kind encapsulating read/write behaviour (§3.8.2) · L10000
- `AccessorMemberDefinition = { Modifier } ( "get" | "set" ) [ ":" Type ] [ "=" Expression ] | { Modifier } "get" "(" [ "this" ":" "&" Type ] ")" Block | { Modifier } "set" "(" [ "this" ":" "&" Type "," ] "value" [ ":" Type ] ")" Block . …` — accessor member definition / block shorthand (§3.8.2) · L10001
- `FunctionTypeConstructor = "function" [ "(" [ ParameterList ] [ ";" CallingConventionExpression ] ")" ] . …` — function-type construction; function types are reference types describing the caller's behaviour (§3.6.3) · L10002
- `PointerTypeConstructor = "pointer" .` — pointer type constructor; pointer types are value types denoting a memory address (§3.6.2) · L10003
- `ArrayTypeConstructor = "array(" Expression { "," Expression } ")" .` — array type constructor; arrays are value types, fixed-length homogeneous sequences (§3.6.1) · L10004
- `ConstructedType = TypeConstructor [ ConstructTargetType ] . …` — construct a compound type from a basic or existing type (§3.6) · L10005
- `Type = ConstructedType | BasicType | Identifier | MonomorphizationExpression .` — form of a constructed type (§3.1) · L10006
- `UnionDeclaration = { Modifier } "union" Name [ GenericParameters ] "{" { UnionMemberDeclaration | ExtensionDeclaration } "}" . …` — value type, same memory viewed different ways, no default allocation; members declared directly, extension blocks may be nested (§3.7.6) · L10007
- `VariantDeclaration = { Modifier } "variant" Name [ GenericParameters ] "{" { VariantMemberDeclaration | ExtensionDeclaration } "}" . …` — value type representing different states, no default allocation; states declared directly, extension blocks may be nested (§3.7.5) · L10008
- `InterfaceDeclaration = { Modifier } "interface" Name [ GenericParameters ] [ BaseList ] Block .` — API contract for extension content; MUST be implemented only by `class`; members declared directly, no extension blocks (§3.7.3) · L10009
- `EnumerationDeclaration = { Modifier } "enumeration" Name ":" BasicType "{" { EnumerationMemberDeclaration | ExtensionDeclaration } "}" . …` — value type mapping identifiers to underlying-type values; members declared directly, extension blocks may be nested (§3.7.4) · L10010
- `ClassDeclaration = { Modifier } "class" Name [ GenericParameters ] [ BaseList ] Block .` — reference type, no default allocation, single inheritance, multiple interfaces, polymorphism, overriding; body holds fields, modules, extension blocks directly (§3.7.1) · L10011
- `StructureDeclaration = { Modifier } "structure" Name [ GenericParameters ] Block .` — value type, no default allocation; body holds fields, modules, extension blocks directly (§3.7.2) · L10012
- `BaseList = ":" ( Identifier | MonomorphizationExpression ) { "," ( Identifier | MonomorphizationExpression ) } .` — inherited/implemented targets (§3.7.7) · L10013

### 3.4 有关表达式 (Expressions) — L10015–L10058

- `IsExpression = Expression "is" IsTarget . …` — form-testing expression (§4.8) · L10017
- `ValueExpression = Identifier …` — expression denoting a value (§4.1) · L10018
- `FunctionCallExpression = Expression "(" Arguments ")" .` — call a function (§4.5) · L10019
- `MemberConstructionExpression = ConstructedTypeName "." Name "(" Expression ")" .` — construct a host instance from a member (§4.7) · L10020
- `ParenthesisExpression = "(" Expression ")" .` — change evaluation precedence (§4.2) · L10021
- `ConstructionExpression = ConstructedTypeName "(" Arguments ")" . …` — construct an instance (§4.6) · L10022
- `TypeConversionExpression = "Cast" "<" Type ">" "(" Expression ")" .` — explicit conversion to a target type (§4.3) · L10023
- `Expression = ValueExpression …` — syntax forms with a value, used for computation (§4) · L10024
- `MemberAccessExpression = ( Identifier | Expression ) "." Name .` — fetch a member of an identifier, type or instance (§4.4.1) · L10025
- `PointerMemberAccessExpression = ( Identifier | Expression ) "->" Name .` — fetch a member of the pointee of an identifier or instance (§4.4.2) · L10026
- `IndexAccessExpression = ( Identifier | Expression ) "[" Expression "]" .` — fetch by index (§4.4.3) · L10027
- `AccessExpression = MemberAccessExpression …` — describes access to an identifier or instance (§4.4) · L10028
- `NotEqualExpression = Expression "!=" Expression .` — inequality (§4.9.20) · L10029
- `MultiplyExpression = Expression "*" Expression .` — product (§4.9.8) · L10030
- `MinusExpression = Expression "-" Expression .` — difference (§4.9.12) · L10031
- `AddExpression = Expression "+" Expression .` — sum (§4.9.11) · L10032
- `NegateExpression = "-" Expression .` — arithmetic negation (§4.9.3) · L10033
- `GreaterEqualExpression = Expression ">=" Expression .` — ≥ comparison (§4.9.18) · L10034
- `GreaterExpression = Expression ">" Expression .` — > comparison (§4.9.16) · L10035
- `LessEqualExpression = Expression "<=" Expression .` — ≤ comparison (§4.9.17) · L10036
- `LessExpression = Expression "<" Expression .` — < comparison (§4.9.15) · L10037
- `ReferenceExpression = "&" Expression .` — address-of (§4.9.6) · L10038
- `BitwiseAndExpression = Expression "&" Expression .` — bitwise AND (§4.9.21) · L10039
- `BitwiseNotExpression = "^" Expression .` — bitwise NOT (§4.9.5) · L10040
- `BitwiseXorExpression = Expression "^" Expression .` — bitwise XOR (§4.9.22) · L10041
- `BitwiseOrExpression = Expression "|" Expression .` — bitwise OR (§4.9.23) · L10042
- `ConditionalExpression = Expression "?" Expression ":" Expression .` — choose between two values by condition (§4.9.27) · L10043
- `RemainderExpression = Expression "%" Expression .` — remainder (§4.9.10) · L10044
- `EqualExpression = Expression "==" Expression .` — equality (§4.9.19) · L10045
- `NullCoalescingExpression = Expression "??" CoalescingAlternative . …` — decide on the left operand with `??` and run the right-hand continuation when the decision is `Alternative` (§4.9.26) · L10046
- `DereferenceExpression = "*" Expression .` — pointee (§4.9.7) · L10047
- `OperatorDefinition = { Modifier } "operator" OperatorHead [ "=" Expression | Block | AccessorMembers ] . …` — identifier kind supplying operator behaviour (§4.9) · L10048
- `OperatorHead = OperatorShorthand | OperatorNotation [ ":" Type ] . …` — operator head (§4.9) · L10049
- `OperatorShorthand = OperatorSymbol "(" [ Parameters ] ")" [ ":" Type ] . …` — operator shorthand (§4.9) · L10050

Pointer-only entries: precedence order §4.9.1 · L10051; operator overload for custom types §4.9.2 · L10052.

- `LogicalRightShiftExpression = Expression ">>" Expression .` — logical right shift (§4.9.14) · L10053
- `LogicalAndExpression = Expression "&&" Expression .` — boolean AND (§4.9.24) · L10054
- `LogicalLeftShiftExpression = Expression "<<" Expression .` — logical left shift (§4.9.13) · L10055
- `LogicalOrExpression = Expression "||" Expression .` — boolean OR (§4.9.25) · L10056
- `LogicalNotExpression = "!" Expression .` — boolean negation (§4.9.4) · L10057
- `DivideExpression = Expression "/" Expression .` — quotient (§4.9.9) · L10058

### 3.5 有关语句 (Statements) — L10060–L10086

- `DisposeStatement = "dispose" ( Identifier | PointerMemberAccessExpression ) .` — destroy heap storage (§5.8) · L10062
- `Block = "{" { Statement | Block } "}" .` — scope holding a statement sequence (§5.1) · L10063
- `CompileTimeIf = "#if" Expression CompileTimeBranch [ "else" CompileTimeBranch ] . …` — compile-time conditional selection (§11.6) · L10064
- `LocalDefinition = { Modifier } "local" LocalItem { "," LocalItem } . …` — identifier kind taking instance storage in the current scope (§5.2) · L10065
- `LocalItem = Name [ ":" Type ] [ "=" Expression ] . …` — local definition item (§5.2) · L10066
- `HeapDefinition = { Modifier } "heap" [ "(" Expression ")" ] Name [ ":" Type ] [ "=" Expression ] .` — identifier kind taking instance storage on the heap (§5.4) · L10067
- `FixedDefinition = { Modifier } "fixed" Name [ GenericParameters ] [ ":" Type ] [ "=" Expression | Block ] .` — identifier kind not assignable after initialization (§5.3) · L10068
- `LinkableDefinition = { Modifier } "linkable" [ "(" StringLiteral ")" ] FunctionShorthand | { Modifier } "linkable" [ "(" StringLiteral ")" ] Name [ GenericParameters ] [ ":" Type ] [ "=" Expression | Block ] . …` — identifier kind guaranteed to be exposed as an external symbol (§5.5) · L10069
- `ReturnStatement = "return" [ Expression ] .` — exit the function by passing a value (§5.10.2) · L10070
- `ThrowStatement = "throw" Expression .` — throw a value inside `trap`/`catch` for local `trap` interception (§5.10.3) · L10071
- `TrapStatement  = "trap" TrapBody { "catch" CatchCondition CatchBlock } [ "finally" FinallyBlock ] . …` — intercept thrown values from this block and nested `trap`s (§5.10.4) · L10072

Pointer-only entry: error/exception handling model §5.10.1 · L10073.

- `Statement = LocalDefinition …` — syntactic unit appearing independently in a block or file body (§5) · L10074
- `ExpressionStatement = FunctionCallExpression .` — evaluate a call and discard its result (§5) · L10075
- `FileBody = { Statement } .` — statement sequence at file top level (§5) · L10076
- `ExtensionDeclaration = "extension" [ "for" ExtensionTarget ] Block .` — (duplicate of L9999) (§3.8.3) · L10077
- `SetStatement          = AssignmentTarget "=" Expression . …` — overwrite the memory of the assignment target (§5.7) · L10078
- `BreakStatement = "break" [ Name ] .` — exit a control flow (§5.9.7) · L10079
- `ContinueStatement = "continue" [ Name ] .` — continue a control flow (§5.9.8) · L10080
- `DoWhileStatement = "do" [ ":" Name ] Block "while" Expression .` — post-test loop (§5.9.4) · L10081
- `ForStatement = "for" [ "&" ] Name ":" Type "in" TraverseTarget [ ":" Name ] Block . …` — iterate an array or iterable (§5.9.5) · L10082
- `IfStatement = "if" Expression Block { "else" "if" Expression Block } [ "else" Block ] .` — conditional block selection (§5.9.2) · L10083
- `MatchStatement  = "match" Expression [ ":" Name ] "{" { MatchCase } "}" . …` — pattern-selected blocks (§5.9.6) · L10084
- `WhileStatement = "while" Expression [ ":" Name ] Block .` — pre-test loop (§5.9.3) · L10085

Pointer-only entry: naming logical control flows §5.9.1 · L10086.

### 3.6 有关函数 (Functions) — L10088–L10100

- `FunctionDeclarationSugar = { Modifier } "fixed" Name [ GenericParameters ] ":" FunctionTypeConstructor Block .` — `Block` in place of `= <function literal>` (§8.1) · L10090
- `Parameters = Parameter { "," Parameter } . …` — parameters of a function type constructor (§8.2) · L10091
- `LiteralParameters = LiteralParameter { "," LiteralParameter } . …` — parameters of a function literal (§8.2) · L10092
- `ParameterList = Parameters [ "," "wildcard" ] | "wildcard" . …` — parameter list of a function type constructor (§3.6.3) · L10093
- `Parameter = [ "#" ] [ Name ":" ] [ "&" ] Type [ "=" Expression ] . …` — function type constructor parameter (§8.2) · L10094
- `LiteralParameter = [ "#" ] Name [ ":" [ "&" ] Type ] . …` — function literal parameter (§8.2) · L10095
- `Arguments = [ Argument { "," Argument } ] . …` — values passed by the caller (§8.3) · L10096

Pointer-only entries: methods (`this` = extension target), §8.4 · L10097.

- `CompileTimeExpression = "#" "(" Expression ")" .` — evaluate at compile time, keep result for runtime (§11.3) · L10098
- `CompileTimeBlock = "#" Block .` — execute at compile time, keep terminal products for runtime (§11.4) · L10099

Pointer-only entry: compile-time process selecting the callee among same-named function constants (overload resolution) §8.5 · L10100.

### 3.7 有关泛型 (Generics) — L10102–L10105

- `MonomorphizationExpression = ( Identifier | Expression ) "<" [ Type ] { ";" [ Type ] } ">" .` — monomorphize a generic identifier (§10.2) · L10104
- `GenericParameters = "<" GenericParameterDefinition { ";" GenericParameterDefinition } ">" . …` — type placeholders on a declaration, for compile-time monomorphization (§10.1) · L10105

### 3.8 有关修饰符 (Modifiers) — L10107–L10124

- `AbstractModifier = "abstract" .` — declare an abstract class (§9.10) · L10109
- `AlignedModifier = "aligned" "(" IntegerLiteral ")" .` — structure default alignment, structure-field alignment, or union-member offset (§9.14) · L10110
- `ImplicitableModifier = "implicitable" .` — allow implicit type conversion (§9.7) · L10111
- `ManualModifier = "manual" .` — exempt a heap allocation unit from NOLAA (§9.9) · L10112
- `OverrideModifier = "override" .` — declare an overriding method (§9.13) · L10113
- `PrivateAccessibilityModifier = "private" .` — accessible only in the declaring scope (§9.3) · L10114
- `ProtectedAccessibilityModifier = "protected" .` — accessible in the declaring scope and derived scopes (§9.4) · L10115
- `PublicAccessibilityModifier = "public" .` — accessible from all scopes (§9.2) · L10116
- `ReadonlyModifier = "readonly" .` — declare a read-only member (§9.6) · L10117
- `ScopedModifier = "scoped" "(" ScopedTarget { "," ScopedTarget } ")" . …` — accessibility-narrowing modifier with named targets (§9.5) · L10118
- `SealedModifier = "sealed" .` — non-inheritable class or non-overridable method (§9.11) · L10119
- `StaticModifier = "static" .` — member associated with the type, not an instance (§9.15) · L10120
- `VerbatimModifier = "verbatim" .` — prohibit compile-time optimization (§9.8) · L10121
- `VirtualModifier = "virtual" .` — declare an overridable method (§9.12) · L10122
- `Modifier = AccessibilityModifier …` — form preceding an identifier kind that limits its behaviour (§9) · L10123
- `AccessibilityModifier = BaseAccessibilityModifier | ScopedModifier . …` — limits which scopes may access an identifier (§9.1) · L10124

### 3.9 有关模块 (Modules) — L10126–L10129

- `UseStatement = "use" Identifier { "," Identifier } . …` — bring target identifiers' members into the current scope (§5.6) · L10128
- `ModuleDeclaration = { Modifier } "module" Name Block .` — namespace organizing identifiers (§7) · L10129

### 3.10 有关内存模型 / 程序 / ABI / 设计基准 / 附录 — L10131–L10155

All pointer-only, **no productions** (eleven entries, in order): NOLAA tool §12.2 · L10133; obtaining memory for an instance §12.1 · L10134; function constant called at program start §14.1 · L10138; execution order from entry point to termination §14.2 · L10139; binary-level interop contract §13.1 · L10143; name-mangling rules §13.2 · L10144; design baseline §1.1–§1.3 · L10148–L10150; appendix pointers §15.2 / §15.3 · L10154–L10155.

### 3.11 Completed productions (ellipsis resolution)

The spec elides 46 productions with `…` in §15.1, and omits several sub-productions entirely. The forms below are **quoted from the governing sections' own EBNF** (extracted in [`_all-ebnf.md`](_all-ebnf.md)); they add no new rules. Use these as the parser's production set; `…` in the category lists above means "see here".

```ebnf
(* Lexical — §2.4, §2.7 *)
Name                  = ( "a" .. "z" | "A" .. "Z" | "_" ) { "a" .. "z" | "A" .. "Z" | "0" .. "9" | "_" } .
Literal               = IntegerLiteral | FloatLiteral | ArrayLiteral | CharacterLiteral | StringLiteral
                      | FunctionLiteral | BooleanLiteral | IllegalLiteral | InfiniteLiteral | UninitializedLiteral .
DecimalLiteral        = Digit { Digit | "_" } .
HexLiteral            = "0x" HexDigit { HexDigit | "_" } .
BinaryLiteral         = "0b" ( "0" | "1" ) { ( "0" | "1" ) | "_" } .
Digit                 = "0" .. "9" .
HexDigit              = Digit | "A" .. "F" | "a" .. "f" .
Exponent              = ( "e" | "E" ) [ "+" | "-" ] DecimalLiteral .
NormalStringLiteral   = '"' { Character | EscapeSequence } '"' [ ":" CharacterEncodingTag ] .
VerbatimStringLiteral = '@"' { Character | '""' } '"' [ ":" CharacterEncodingTag ] .
CharacterEncodingTag  = "ascii" | "utf8" | "utf16" | "utf32" | "ucs2" | "ucs4" .
EscapeSequence        = "\" ( "a" | "b" | "f" | "n" | "r" | "t" | "v" | "\" | '"' | "'" | "0" )
                      | "\x" HexDigit HexDigit
                      | "\u" HexDigit HexDigit HexDigit HexDigit
                      | "\U" HexDigit HexDigit HexDigit HexDigit HexDigit HexDigit HexDigit HexDigit .
Element               = Expression | RepeatElement | RangeElement .
RepeatElement         = "[" Expression ":" Expression "]" .
RangeElement          = "[" Expression [ ".." Expression ] ( ".." | "..!" ) Expression "]" .
(* Types — §3.2, §3.6, §3.7, §3.8 *)
IntegerType           = "integer" "(" IntegerLiteral ")" .
CardinalType          = "cardinal" "(" IntegerLiteral ")" .
FloatType             = "float" "(" IntegerLiteral ")" .
BooleanType           = "boolean" .
TypeConstructor       = ArrayTypeConstructor | FunctionTypeConstructor | PointerTypeConstructor .
ConstructTargetType   = BasicType | Type .
ParameterList         = Parameters [ "," "wildcard" ] | "wildcard" .
CallingConventionExpression = Expression .
EnumerationMemberDeclaration = Name "=" Expression ";" .
VariantMemberDeclaration     = Name [ ":" Type ] ";" .
UnionMemberDeclaration       = { Modifier } Name ":" Type ";" .
AccessorMembers       = "{" [ AccessorMemberDefinition { ";" AccessorMemberDefinition } [ ";" ] ] "}" .
Backing               = Identifier .
ExtensionTarget       = Type | GenericTarget | ArrayTarget .
GenericTarget         = Identifier "<" GenericParameterDefinition { ";" GenericParameterDefinition } ">" .
ArrayTarget           = "array" "(" TargetBinding { "," TargetBinding } ")" GenericParameterDefinition .
TargetBinding         = Name ":" Type .
(* Expressions — §4, §4.4, §4.6, §4.8, §4.9, §4.9.26 *)
Expression            = ValueExpression | ParenthesisExpression | TypeConversionExpression
                      | AddExpression | MinusExpression | MultiplyExpression | DivideExpression
                      | RemainderExpression | LogicalNotExpression | LogicalAndExpression
                      | LogicalOrExpression | EqualExpression | NotEqualExpression | GreaterExpression
                      | LessExpression | GreaterEqualExpression | LessEqualExpression
                      | BitwiseNotExpression | BitwiseAndExpression | BitwiseOrExpression
                      | BitwiseXorExpression | LogicalRightShiftExpression | LogicalLeftShiftExpression
                      | ReferenceExpression | DereferenceExpression | NegateExpression
                      | FunctionCallExpression | ConstructionExpression | MemberConstructionExpression
                      | IsExpression | AccessExpression | ConditionalExpression
                      | NullCoalescingExpression | CompileTimeExpression .
ValueExpression       = Identifier | Literal | MemberAccessExpression | PointerMemberAccessExpression
                      | MonomorphizationExpression | IndexAccessExpression .
AccessExpression      = MemberAccessExpression | PointerMemberAccessExpression
                      | MonomorphizationExpression | IndexAccessExpression .
ConstructedTypeName   = Type | MonomorphizationExpression .
IsTarget              = Identifier | SpecialFloatTarget .
SpecialFloatTarget    = IllegalLiteral | InfinityTarget .
InfinityTarget        = [ "-" ] "infinite" .
CoalescingAlternative = Expression | "throw" [ Expression ] | "return" [ Expression ] .
OperatorHead          = OperatorShorthand | OperatorNotation [ ":" Type ] .
OperatorNotation      = OperatorSymbol | IndexerNotation | ConstructedTypeName .
OperatorSymbol        = "+" | "-" | "*" | "/" | "%" | "==" | ">" | "<" | "??" .
IndexerNotation       = "[" Parameters "]" .
(* Statements — §5, §5.7, §5.9, §5.10.4 *)
Statement             = LocalDefinition | HeapDefinition | OperatorDefinition | FixedDefinition
                      | LinkableDefinition | FieldDefinition | AccessorDefinition | SetStatement
                      | CompoundSetStatement | UseStatement | ModuleDeclaration | ClassDeclaration
                      | StructureDeclaration | InterfaceDeclaration | EnumerationDeclaration
                      | VariantDeclaration | UnionDeclaration | ExtensionDeclaration
                      | CompileTimeBlock | CompileTimeIf | IfStatement | WhileStatement
                      | DoWhileStatement | ForStatement | MatchStatement | BreakStatement
                      | ContinueStatement | ReturnStatement | ThrowStatement | TrapStatement
                      | DisposeStatement | ExpressionStatement .
CompoundSetStatement  = AssignmentTarget ( "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ) Expression .
AssignmentTarget      = Identifier | IndexAccessExpression | PointerMemberAccessExpression .
LogicalControlFlowName = ":" Name .
TraverseTarget        = Expression | Range .
Range                 = Expression [ ".." Expression ] ( ".." | "..!" ) Expression .
MatchCase             = "case" ( Pattern [ Guard ] | Guard ) Block .
Pattern               = EqualPattern | ComparisonPattern | IsPattern | WildcardPattern .
EqualPattern          = Expression .
ComparisonPattern     = ComparisonOperator Expression .
ComparisonOperator    = "!=" | ">" | "<" | ">=" | "<=" .
IsPattern             = "is" [ Name ":" ] IsTarget .
WildcardPattern       = "wildcard" .
Guard                 = "when" Expression .
TrapBody              = Block .
CatchBlock            = Block .
FinallyBlock          = Block .
CatchCondition        = [ Name ":" ] Type | "wildcard" .
(* Functions — §8.2, §8.3 *)
Parameter             = [ "#" ] [ Name ":" ] [ "&" ] Type [ "=" Expression ] .
LiteralParameter      = [ "#" ] Name [ ":" [ "&" ] Type ] .
Argument              = [ Name ":" ] Expression .
(* Generics — §10.1 *)
GenericParameterDefinition = GenericParameter [ ":" Constraint { "," Constraint } ] .
GenericParameter      = Identifier .
Constraint            = TypeKind | Identifier .
TypeKind              = "class" | "structure" | "interface" | "enumeration" | "variant" | "union" | "basic" .
(* Modifiers — §9, §9.1, §9.5 *)
Modifier              = AccessibilityModifier | BehaviorLimitingModifier | BehaviorEnablingModifier
                      | OptimizationModifier | OtherModifier .
BehaviorLimitingModifier = ReadonlyModifier .
BehaviorEnablingModifier = ImplicitableModifier .
OptimizationModifier  = VerbatimModifier .
OtherModifier         = ManualModifier | AbstractModifier | SealedModifier | VirtualModifier
                      | OverrideModifier | AlignedModifier | StaticModifier .
BaseAccessibilityModifier = PublicAccessibilityModifier | PrivateAccessibilityModifier
                      | ProtectedAccessibilityModifier .
ScopedTarget          = [ "sealed" ] Identifier | "operator" [ Identifier | OperatorSymbol ] .
(* Modules — §5.6 *)
UseStatement          = "use" Identifier { "," Identifier } .
```

Notes for the parser: (a) §15.1 never lists `CompoundSetStatement`, `AssignmentTarget`, `TraverseTarget`, `Range`, `MatchCase`/`Pattern` family, `TrapBody`/`CatchCondition`, `LogicalControlFlowName`, `CallingConventionExpression`, `TargetBinding`, `Element`/`RepeatElement`/`RangeElement` — yet §5/§4/§8 require them; they are listed above; (b) `ExtensionDeclaration` appears twice in §15.1 (L9999, L10077) with identical text; (c) `Any`-style ordering constraints (e.g. `"#"` immediately followed by `(`/`{`) are lexical, not grammar, and are stated in §11.1/§11.3/§11.4/§11.6.

## 4. §15.2 Built-In Library 完整内容 (L10157–L10359)

§15.2 DEFINES the complete set of definitions the Built-In Library must ship. It is normative code: 13 top-level definitions (8 classes, 3 interfaces, 1 enumeration, 2 variants). Reproduced verbatim below (L10161–L10359, 199 lines) — **do not truncate**.

```novasharp
/// @brief Represents the root class of the inheritance hierarchy.
abstract class Object
{

}

/// @brief Identifies the calling convention of a function type.
enumeration CallingConvention: integer(4)
{
    /// @brief The platform's default calling convention.
    PlatformDefault = 0;
    /// @brief The C declaration calling convention.
    CDeclaration = 1;
    /// @brief The standard calling convention.
    StandardCall = 2;
    /// @brief The fast calling convention.
    FastCall = 3;
    /// @brief The System V AMD64 calling convention.
    SystemVAMD64 = 4;
    /// @brief The this-call calling convention.
    ThisCall = 5;
    /// @brief The vector calling convention.
    VectorCall = 6;
    /// @brief The Microsoft x64 calling convention.
    MicrosoftX64 = 7;
}

/// @brief Serves as the base class of all exceptions.
class Exception
{
    /// @brief Holds the exception that caused this exception, if any.
    public InnerException: Optional<Exception>;

    extension
    {
        /// @brief Initializes the exception.
        /// @param this The exception being initialized.
        /// @param innerException The exception that caused this exception.
        public fixed Construct(this: Exception, innerException: Optional<Exception> = Optional<Exception>.Null)
        {
            this.InnerException = innerException;
        }
    }
}

/// @brief Thrown when a requested operation is not implemented.
class NotImplementedException: Exception
{
    extension
    {
        /// @brief Initializes the exception.
        /// @param this The exception being initialized.
        public fixed Construct(this: NotImplementedException)
        {
            base.Construct(innerException: Optional<Exception>.Null);
        }
    }
}

/// @brief Thrown when an integer division or remainder has a zero divisor.
class DivideByZeroException: Exception
{
    extension
    {
        /// @brief Initializes the exception.
        /// @param this The exception being initialized.
        /// @param innerException The exception that caused this exception.
        public fixed Construct(this: DivideByZeroException, innerException: Optional<Exception> = Optional<Exception>.Null)
        {
            base.Construct(innerException: innerException);
        }
    }
}

/// @brief Thrown when an index is outside the bounds of an array.
class OutOfRangeException: Exception
{
    extension
    {
        /// @brief Initializes the exception.
        /// @param this The exception being initialized.
        /// @param innerException The exception that caused this exception.
        public fixed Construct(this: OutOfRangeException, innerException: Optional<Exception> = Optional<Exception>.Null)
        {
            base.Construct(innerException: innerException);
        }
    }
}

/// @brief Thrown when memory allocation fails.
class AllocateFailedException: Exception
{
    extension
    {
        /// @brief Initializes the exception.
        /// @param this The exception being initialized.
        public fixed Construct(this: AllocateFailedException)
        {
            base.Construct(innerException: Optional<Exception>.Null);
        }
    }
}

/// @brief Thrown when memory deallocation fails.
class DeallocateFailedException: Exception
{
    extension
    {
        /// @brief Initializes the exception.
        /// @param this The exception being initialized.
        public fixed Construct(this: DeallocateFailedException)
        {
            base.Construct(innerException: Optional<Exception>.Null);
        }
    }
}

/// @brief Iterates over the elements of a sequence.
/// @tparam T The type of the elements.
interface IIterator<T>
{
    /// @brief Advances to the next element.
    /// @return The next element, or Null when there are no more elements.
    public fixed Next(this: IIterator<T>): Optional<T>;

    /// @brief Returns the iterator to its initial position.
    public fixed Reset(this: IIterator<T>);
}

/// @brief Exposes an iterator over the elements of a sequence.
/// @tparam T The type of the elements.
interface IIterable<T>
{
    /// @brief Gets an iterator over the elements.
    public accessor Iterator: IIterator<T> { get; }
}

/// @brief Provides heap allocation and deallocation.
interface IAllocator
{
    /// @brief Allocates memory.
    /// @param this The allocator.
    /// @param size The requested size in bytes.
    /// @param alignment The requested alignment in bytes.
    /// @return The allocated pointer, or an allocation failure.
    public fixed Allocate(this: IAllocator, size: cardinal(8), alignment: cardinal(8)): Result<pointer; AllocateFailedException>;

    /// @brief Deallocates memory.
    /// @param this The allocator.
    /// @param block The block to deallocate.
    /// @return An empty result, or a deallocation failure.
    public fixed Deallocate(this: IAllocator, block: pointer): Result<; DeallocateFailedException>;
}

/// @brief Represents an optional value.
variant Optional<T>
{
    Value: T;
    Null;

    extension
    {
        /// @brief Maps Optional to Result for ??.
        /// @param value The optional value.
        /// @return A Result whose Value carries T, or whose Alternative is empty.
        operator ??(value: Optional<T>): Result<T;>
        {
            match value
            {
                case is payload:Optional<T>.Value:
                    return Result<T;>.Value(payload);
                case is Optional<T>.Null:
                    return Result<T;>.Alternative;
            }
        }
    }
}

/// @brief Represents a primary value or an alternative.
/// @tparam TValue The primary value type.
/// @tparam TAlternative The alternative type.
variant Result<TValue; TAlternative>
{
    Value: TValue;
    Alternative: TAlternative;

    extension
    {
        /// @brief Returns the result itself as the ?? decision.
        /// @param value The result to decide on.
        /// @return The same result.
        operator ??(value: Result<TValue; TAlternative>): Result<TValue; TAlternative>
        {
            return value;
        }
    }
}
```

### 4.1 Inventory (exact signatures)

Classes (all reference types; no default allocation per §12.1 L9425):

| Definition | Kind | Base | Members (exact) | Lines |
| --- | --- | --- | --- | --- |
| `Object` | `abstract class` | — | none | L10163–L10166 |
| `Exception` | `class` | — | field `public InnerException: Optional<Exception>;`; ext. `public fixed Construct(this: Exception, innerException: Optional<Exception> = Optional<Exception>.Null)` | L10190–L10205 |
| `NotImplementedException` | `class` | `Exception` | ext. `public fixed Construct(this: NotImplementedException)` | L10208–L10219 |
| `DivideByZeroException` | `class` | `Exception` | ext. `public fixed Construct(this: DivideByZeroException, innerException: Optional<Exception> = Optional<Exception>.Null)` | L10222–L10234 |
| `OutOfRangeException` | `class` | `Exception` | ext. `public fixed Construct(this: OutOfRangeException, innerException: Optional<Exception> = Optional<Exception>.Null)` | L10236–L10249 |
| `AllocateFailedException` | `class` | `Exception` | ext. `public fixed Construct(this: AllocateFailedException)` | L10252–L10263 |
| `DeallocateFailedException` | `class` | `Exception` | ext. `public fixed Construct(this: DeallocateFailedException)` | L10266–L10277 |

Enumeration `CallingConvention: integer(4)` (underlying `integer(4)`, values 0–7) — members table:

| Member | Value | Spec gloss |
| --- | --- | --- |
| `PlatformDefault` | 0 | platform default calling convention |
| `CDeclaration` | 1 | C declaration calling convention |
| `StandardCall` | 2 | standard calling convention |
| `FastCall` | 3 | fast calling convention |
| `SystemVAMD64` | 4 | System V AMD64 |
| `ThisCall` | 5 | this-call |
| `VectorCall` | 6 | vector call |
| `MicrosoftX64` | 7 | Microsoft x64 |

Interfaces (API contracts; may be implemented only by `class` — §15.1 L10009):

| Interface | Members (exact) | Lines |
| --- | --- | --- |
| `IIterator<T>` | `public fixed Next(this: IIterator<T>): Optional<T>;` · `public fixed Reset(this: IIterator<T>);` | L10281–L10289 |
| `IIterable<T>` | `public accessor Iterator: IIterator<T> { get; }` (get-only ⇒ read-only) | L10293–L10297 |
| `IAllocator` | `public fixed Allocate(this: IAllocator, size: cardinal(8), alignment: cardinal(8)): Result<pointer; AllocateFailedException>;` · `public fixed Deallocate(this: IAllocator, block: pointer): Result<; DeallocateFailedException>;` | L10300–L10314 |

Variants (value types; no default allocation):

| Variant | Members (exact) | Operator | Lines |
| --- | --- | --- | --- |
| `Optional<T>` | `Value: T;` · `Null;` (payload-less member) | `operator ??(value: Optional<T>): Result<T;>` → `Value(payload)` ⇒ `Result<T;>.Value`, `Null` ⇒ `Result<T;>.Alternative` | L10317–L10338 |
| `Result<TValue; TAlternative>` | `Value: TValue;` · `Alternative: TAlternative;` | `operator ??(value: Result<TValue; TAlternative>): Result<TValue; TAlternative>` → identity | L10343–L10358 |

### 4.2 Semantics given by §15.2

- `Object` is the root of the inheritance hierarchy (L10162). No members, `abstract` (L10163).
- `CallingConvention` is an `enumeration` over `integer(4)` (L10169); its members identify a function type's calling convention (L10168); §13.1 delegates "调用约定" to §15.2 (L9630), so this enumeration is the spec's only calling-convention model.
- `Exception` holds `InnerException: Optional<Exception>` (L10192–L10193), initialised by `Construct` assigning `this.InnerException = innerException` (L10200–L10203). Default `innerException` is `Optional<Exception>.Null` (L10200).
- Derived exception constructors re-enter the base with `base.Construct(innerException: …)` (L10216, L10231, L10246, L10260, L10274) — named-argument syntax (`Argument = [ Name ":" ] Expression`, §8.3).
- `NotImplementedException`, `AllocateFailedException`, `DeallocateFailedException` take no `innerException` parameter and hard-code `Optional<Exception>.Null` (L10214–L10217, L10258–L10261, L10272–L10275).
- `IIterator<T>.Next` returns `Optional<T>`: `Null` means exhausted (L10284–L10285). `Reset` repositions to the start (L10287–L10288).
- `IIterable<T>` exposes `Iterator` as a get-only `accessor` (L10295–L10296) — this is the iterator contract `for` consumes (see §5.9.5 `ForStatement`).
- `IAllocator.Allocate` takes `size`/`alignment` in bytes and returns `Result<pointer; AllocateFailedException>`; `Deallocate` takes a `block: pointer` and returns `Result<; DeallocateFailedException>` (empty primary type) (L10307, L10313).
- `Optional<T>` is the language's optional value; its `??` operator exists **only** to map to `Result<T;>` so that `??` can then decide (L10324–L10336). `Result<TValue; TAlternative>`'s `??` returns itself, making `Result` already a decision (L10350–L10356).
- Empty type arguments are written by omitting the type: `Result<T;>` (TValue = `T`, TAlternative = empty), `Result<; AllocateFailedException>` (TValue = empty) — matches `MonomorphizationExpression = … "<" [ Type ] { ";" [ Type ] } ">"` (§15.1 L10104).
- `Construct` is declared as an ordinary `public fixed` function inside `extension` blocks, **not** with a `constructor`-specific keyword; `Cat()` style construction in §12.1 (L9493) therefore targets the type's construction function set.

## 5. §15.3 Compile-Time Library 完整内容 (L10361–L10421)

§15.3 DEFINES the standard definitions the compiler provides in the compile-time code space. The compiler MAY define non-standard compile-time APIs (§11.8, L10365). Verbatim (L10367–L10421):

```novasharp
/// @brief Exposes compile-time type queries.
module Type
{
    /// @brief Returns the size of T in bytes.
    /// @return A value whose width equals the target pointer width.
    public linkable SizeOf<T>(): cardinal(8);
}

/// @brief Exposes compile-time diagnostics.
module Compiler
{
    /// @brief Emits a compile-time note.
    /// @param message The message bytes.
    /// @param length The message byte length.
    public linkable LogNote(message[length: cardinal(4)]: cardinal(1));

    /// @brief Emits a compile-time warning.
    /// @param message The message bytes.
    /// @param length The message byte length.
    public linkable LogWarning(message[length: cardinal(4)]: cardinal(1));

    /// @brief Emits a compile-time error.
    /// @param message The message bytes.
    /// @param length The message byte length.
    public linkable LogError(message[length: cardinal(4)]: cardinal(1));

    /// @brief Asserts a compile-time condition.
    /// @param condition The condition to assert.
    public linkable Assert(condition: boolean);

    /// @brief Asserts a compile-time condition with a message.
    /// @param condition The condition to assert.
    /// @param message The message bytes.
    /// @param length The message byte length.
    public linkable Assert(condition: boolean, message[length: cardinal(4)]: cardinal(1));
}

/// @brief Exposes heap lifetime analysis controls.
module NOLAA
{
    /// @brief Stops automatic analysis of the pointed heap object.
    /// @param x The heap object pointer.
    public linkable Abandon(x: pointer);

    /// @brief Requires required to exist while requiredBy exists.
    /// @param required The heap object that must exist.
    /// @param requiredBy The heap object that needs required.
    public linkable Bind(required: pointer, requiredBy: pointer);

    /// @brief Assumes the next statement does not require x.
    /// @param x The heap object pointer.
    public linkable AssumeNoDemandNext(x: pointer);
}
```

### 5.1 Inventory (exact signatures)

Module `Type` (L10369–L10374):

| Function | Signature | Contract |
| --- | --- | --- |
| `SizeOf` | `public linkable SizeOf<T>(): cardinal(8)` | returns `T`'s size in bytes; "a value whose width equals the target pointer width" (L10371–L10372); §11.7: MUST return `T`'s memory size (L9368) |

Module `Compiler` (L10377–L10403) — all return the empty type:

| Function | Signature | Doc-comment semantics |
| --- | --- | --- |
| `LogNote` | `public linkable LogNote(message[length: cardinal(4)]: cardinal(1))` | emits a compile-time note (L10379) |
| `LogWarning` | `public linkable LogWarning(message[length: cardinal(4)]: cardinal(1))` | emits a compile-time warning (L10384) |
| `LogError` | `public linkable LogError(message[length: cardinal(4)]: cardinal(1))` | emits a compile-time error (L10389) |
| `Assert` | `public linkable Assert(condition: boolean)` | asserts a compile-time condition (L10394–L10395) |
| `Assert` | `public linkable Assert(condition: boolean, message[length: cardinal(4)]: cardinal(1))` | asserts with message (L10398–L10402) |

Overload pair `Assert/1` and `Assert/2` is the only overload set in the compile-time library; resolution follows §8.5, and both bindings share the name `Assert`.

Module `NOLAA` (L10406–L10420) — all take `pointer` and return the empty type:

| Function | Signature | Contract (§15.3 / §12.2) |
| --- | --- | --- |
| `Abandon` | `public linkable Abandon(x: pointer)` | stops automatic analysis of the pointed heap object (L10408–L10410); after it: no `dispose` insertion and no Never-Disposed warning (L9551–L9552) |
| `Bind` | `public linkable Bind(required: pointer, requiredBy: pointer)` | adds an existence demand: while `requiredBy` exists, `required` MUST NOT be released (L10412–L10415, L9553–L9555); the demand participates in SCC determination (L9555) |
| `AssumeNoDemandNext` | `public linkable AssumeNoDemandNext(x: pointer)` | applies to exactly the immediately following statement only; that statement MUST NOT create a new existence demand on `x`; if it is a `linkable` call, NOLAA MUST NOT abandon because of it; normal analysis resumes afterwards (L10417–L10419, L9556–L9559) |

### 5.2 Contract imposed by §11.7/§11.8 on all compile-time APIs

- Compiler-API functions MUST be `linkable` functions (L9365); their function values MUST NOT enter runtime (L9366).
- `Type`, `Compiler`, `NOLAA` MUST be namespaces in the compile-time code space (L9367). §15.3 spells them `module` (L10369, L10377, L10406) — see Q6.
- `Compiler` and `NOLAA` return types MUST be the empty type (L9370) — consistent with all six `Compiler` functions and all three `NOLAA` functions returning nothing.
- `NOLAA` function parameters MUST designate heap objects trackable by NOLAA (L9371); a parameter that is not a trackable heap object is a compile error (L9560–L9561).
- `Type` return values MAY enter runtime (L9369); `Type.SizeOf<T>()`'s `cardinal(8)` result is therefore usable in runtime initialisers, e.g. `local size: cardinal(8) = #(Type.SizeOf<Vector3>());` (L9085).
- Implementations MAY add compile-time APIs beyond the standard, but MUST NOT shadow or re-specify standard names/semantics, MUST document them, and programs using them are not portable (L9401–L9404).
- `message[length: cardinal(4)]: cardinal(1)` is an array type with a **named** length binding `length` (byte-count of the array, `cardinal(4)`) over element type `cardinal(1)`; it is the array shorthand (`ArrayShorthand`, §15.1 L9998) plus a length binding, and `length` is then a parameter of the function. See Q7.

## 6. Semantic rules (bullets, anchored)

### 6.1 §11.1 编译期执行 (L8953–L8972)

- Compile-time execution is `#`-prefixed execution evaluated by the compiler (L8955).
- After `#`, **only** two forms are allowed: `#(Expression)` (compile-time expression, §11.3) and `#{ Block }` (compile-time block, §11.4) (L8957–L8960).
- Between `#` and `(` **or** `{` there MUST be no whitespace (L8964).
- `#` MUST NOT be used as a type prefix (L8965).
- `#[]` is not a form of this language (L8966).
- Cross-references: compile-time constant parameters §11.5 (L8970), compile-time constants/variables/runtime boundary §11.7 (L8971), multi-file execution order §11.2 (L8972).

### 6.2 §11.2 编译期执行顺序 (L8974–L9031)

- Compile-time execution order is the order of top-level compile-time code and function-body compile-time code under multi-file compilation (L8976).
- Whole-program parse: all source files MUST be inputs to the same compilation (L8980); types, functions, constants and `use` imports MUST all be visible in that compilation (L8981); `use` MUST NOT change execution order — it only affects name resolution (L8982); top-level `#{}` MUST execute after **all selected** declarations have completed name resolution (L8983); declarations in unselected branches MUST NOT participate in name resolution (L8984).
- Top-level execution: file order MAY be given by the build input (L8988); when the build input gives no file order, the compiler MUST use a fixed order, e.g. path sort (L8989); within a file, execution MUST follow source order (L8990); top-level and module-top-level `#{}` MUST each execute exactly once (L8991); multiple top-level `#{}` MUST execute in file order then source order (L8992).
- Embedded compile-time expressions: `#()` in top-level definition initialisers MUST participate in the top-level execution order (L8996); top-level `#()` MUST execute after all declarations are name-resolved (L8997); within one file, top-level `#()` MUST interleave with top-level `#{}` in source order (L8998); across files, top-level `#()` MUST follow file order (L8999); later top-level `#()` MAY read compile-time variables produced by earlier execution (L9000); an earlier top-level `#()` MUST NOT depend on initialisation not yet executed (L9001).
- Cross-file initialisation: later top-level `#{}` MAY read compile-time variables produced by earlier top-level `#{}` (L9005); earlier `#{}` MUST NOT depend on not-yet-executed initialisation (L9006); declarations in all files MAY be referenced by any top-level `#{}` (L9007).
- Conditional compilation: top-level `#if` MUST be evaluated interleaved with top-level `#{}` in file order then source order (L9011); the top-level `#if` condition MUST be evaluated on compile-time bindings already evaluated before it (L9012); a top-level `#if` MUST NOT depend on declarations inside its branches (L9013); the selected branch MUST enter subsequent declaration collection and name resolution (L9014); the unselected branch MUST skip name resolution, type checking and declaration collection (L9015); `#if` in a function body MUST be evaluated when that body is compiled (L9016); when a function is monomorphized by generic parameters **or** compile-time constant parameters, body `#if` MUST be evaluated per instance (L9017).
- Function-body execution: a function declaration MUST NOT trigger execution of body `#{}`/`#()` (L9021); body `#{}`/`#()` MUST execute when the enclosing body is compiled (L9022); when a function is monomorphized by compile-time constant arguments, each instance's `#{}`/`#()` MUST execute separately (L9023); a runtime call MUST NOT trigger evaluation of `#{}`/`#()` (L9024); when a top-level `#{}` calls a function, the body execution MUST happen at that body's compile time (L9025).
- Residuals: the terminal products of top-level and body `#{}` MUST be preserved to runtime (L9029); runtime MUST NOT replay compile-time execution order (L9030); runtime MUST NOT access compile-time bindings (L9031).
- Example (L9035–L9047): `A.ns` sets `Base`, `B.ns` computes `Derived = Base + 1`; with file order A then B, `Base` is initialised before `Derived`.

### 6.3 §11.3 编译期表达式 (L9049–L9089)

- `CompileTimeExpression` DEFINES the compile-time expression (L9059); `Expression` is the evaluated expression (§4) (L9060); no whitespace between `#` and `(` (L9061).
- Evaluation: `Expression` MUST be determinable at compile time (L9065); if not determinable, the compiler MUST report an error (L9066); the final result MUST be preserved to runtime (L9067).
- Result: the final result's type MUST be a runtime type (L9071); if the result is a compile-time-defined type the compiler MUST report an error (L9072); if the result is a `linkable` function the compiler MUST report an error (L9073); if the result is an ordinary compile-time function, that function value MAY enter runtime (L9074).
- Boundary: see §11.7 for the crossing rules including side effects (L9078–L9079).
- Examples: legal `local size: cardinal(8) = #(Type.SizeOf<Vector3>());`; illegal `#(Type.SizeOf<Vector3>)` because the result is a `linkable` function (L9085–L9088).

### 6.4 §11.4 编译期块 (L9091–L9156)

- `CompileTimeBlock` DEFINES the compile-time block (L9101); `Block` is the executed block (§5.1) (L9102); no whitespace between `#` and `{` (L9103).
- Execution: `#{ Block }` MUST execute at compile time (L9107); statements inside MUST execute in ordinary control-flow order (L9108); multiple top-level `#{}` order per §11.2 (L9109); `#if` selects a branch and executes no code (L9110).
- Scope: `#{}` MUST NOT constitute a scope (L9114); its declarations MUST belong to the enclosing compile-time scope (L9115); its `use` MUST take effect in the enclosing compile-time scope (§5.6) (L9116); compile-time variables MAY be assigned only at compile time and runtime keeps only their final value (§11.7) (L9117).
- Placement: `#{}` MAY appear directly at file top level with no enclosing block (L9121); MAY appear in module bodies and function bodies (L9122); `return`, `break`, `continue` inside `#{}` MUST bind to the enclosing function or loop (L9123).
- Control flow: when execution reaches incomplete control flow, that control flow MUST be preserved to runtime in minimal form (L9127); if it cannot be preserved, the compiler MUST report an error (L9128).
- Terminal products: products needed by runtime MUST be preserved to runtime (L9132); if a terminal product is a compile-time-defined type the compiler MUST report an error (L9133); if it is a `linkable` function value the compiler MUST report an error (L9134); an ordinary compile-time function value MAY be preserved to runtime (L9135).
- Side effects: compile-time execution MAY have side effects (L9139); side effects MUST NOT enter runtime terminal products **and** the compiler MUST warn (L9140); compile-time allocation units handled per §11.7 (L9141).
- Example: `#{ local X: integer(4) = 1; X = 2; }` then `local value: integer(4) = X;` is legal (`X`'s final value becomes a runtime constant); a later runtime `X = 3;` is illegal (L9145–L9155).

### 6.5 §11.5 编译期常量参数 (L9158–L9221)

- A compile-time constant parameter is a parameter marked `#` requiring the caller to pass a compile-time constant (L9160); named `#Name: Type`, unnamed `#Type` (L9164–L9165).
- `#` MUST precede the parameter name or the parameter type (L9169); when a name is present, `#` MUST precede the name (L9170); when the name is omitted, `#` MUST precede the type (L9171); `#` is not part of the name nor of the type (L9172); a `this` parameter MUST NOT carry `#` (L9173); parameter type, `&` and default-value rules per §8.2 (L9174).
- Calls: the argument passed MUST be a compile-time constant (L9178); if it is not, the compiler MUST report an error (L9179).
- Types: the `#` convention MUST participate in function-type equality (§3.6.3) (L9183); MUST participate in overload signatures (§8.5) (L9184).
- Monomorphization: a function MUST be monomorphized per compile-time constant argument (L9188); multiple compile-time constant parameters MUST be monomorphized per argument combination (L9189); merging of monomorphized versions MUST follow §14.3.1 (L9190).
- Function body: a compile-time constant parameter is a compile-time binding inside the body (L9194); runtime MUST NOT access that binding (L9195); compile-time blocks MAY reference it (L9196); generic parameters are always compile-time constants (§10.1) (L9197).
- Example: `fixed Apply(#value: integer(4)): integer(4) { #{ return value + 1; } }`; `Apply(1)` legal, `Apply(RuntimeValue())` illegal (L9203–L9214).
- Function types may write `function(#integer(4), #integer(4))` (L9219–L9220).

### 6.6 §11.6 条件编译 (L9223–L9300)

- `CompileTimeIf` DEFINES conditional compilation (L9235); `Expression` is the condition (§4.9.27) (L9236); `CompileTimeBranch` DEFINES the branch (L9237); `Block` is the branch content (§5.1) (L9238); no whitespace between `#` and `if` (L9239); `else` MUST NOT carry `#` (L9240).
- Condition: MUST be a compile-time constant (L9244); its type MUST be `boolean` or implicitly convertible to `boolean` (L9245); MAY reference compile-time bindings evaluated before it (L9246); MUST NOT reference names declared in either branch (L9247); MUST NOT depend on bindings produced by later `#{}`, `#()` or `#if` (L9248).
- Branches: both branches MUST be parsed syntactically (L9252); syntax errors in the unselected branch MUST be reported (L9253); the unselected branch MUST NOT enter name resolution, type checking or declaration collection (L9254); MUST NOT produce runtime code (L9255); the selected branch's declarations MUST enter the enclosing scope (L9256); a branch MUST NOT constitute a scope (L9257); declarations in a branch MUST obey the declaration rules of their position (L9258); `use` in a branch MUST take effect in the enclosing scope (L9259).
- Placement: `#if` MAY appear at file top level, module body, type body, extension block, or function body (L9263); MAY appear inside another `#if` branch (L9264); a top-level `#if` MUST NOT require an enclosing block (L9265); `#if` MUST NOT be used as an expression (L9266).
- Execution: the condition MUST be evaluated at compile time (L9270); the selected branch MUST enter subsequent compilation (L9271); the unselected branch MUST be skipped (L9272); top-level `#if` MUST interleave with top-level `#{}` in file then source order (L9273); body `#if` MUST be evaluated when its body is compiled (L9274); per-instance evaluation on monomorphization by generic parameters or compile-time constant parameters (L9275); runtime MUST NOT evaluate the condition (L9276).
- Example (L9280–L9300): `fixed UseWindows: boolean = true;` then `#if UseWindows { use WindowsPlatform; } else { use PosixPlatform; }`.

### 6.7 §11.7 编译期边界 (L9302–L9391)

- Compile-time constants: a compile-time constant is an identifier or expression whose value is determined at compile time (L9308). It MUST come from one of: a literal (L9312); a generic parameter (L9313); a compile-time constant parameter (L9314); a value produced in `#{}` (L9315); a constant whose initialiser is compile-time evaluable (§5.3) (L9316). An ordinary `local` MUST NOT be a compile-time constant (L9318).
- Compile-time variables: `local` declared in `#{}` MUST be a compile-time variable (L9322); assignable only at compile time (L9323); its final value MAY be preserved to runtime as a runtime constant (L9324); runtime MUST NOT assign it (L9325).
- Runtime → compile time: the compiler MAY read runtime definitions (L9329); when compile-time code references a runtime identifier, that identifier MUST be a compile-time constant (L9330); if not, the compiler MUST report an error (L9331).
- Compile time → runtime: runtime-typed values MAY enter runtime (L9335); a compile-time variable's final value MAY enter runtime as a runtime constant (L9336); ordinary compile-time function values MAY enter runtime (L9337); compile-time-defined types MUST NOT enter runtime (L9338); `linkable` function values MUST NOT enter runtime (L9339).
- Compile-time functions: DEFINES a compile-time function as a function in the compile-time code space (L9341); compile-time functions include ordinary compile-time functions and `linkable` functions (L9343); ordinary compile-time functions MAY enter runtime as values (L9344); `linkable` functions MUST NOT enter runtime as values, **but** their return values MAY enter runtime (L9345).
- Side effects: compile-time execution MAY have side effects (L9349); side effects MUST NOT enter runtime terminal products (L9350); when compile-time execution produces side effects the compiler MUST warn (L9351).
- Allocation: allocation units in compile-time execution MUST be released when that execution ends (L9355); MUST NOT enter runtime (L9356); NOLAA MUST NOT govern compile-time allocation units (§12.2) (L9357).
- Size optimization: compile-time residual size cleanup per §14.3.1 (L9361).
- Compiler API: API functions MUST be `linkable` (L9365); their function values MUST NOT enter runtime (L9366); `Type`, `Compiler`, `NOLAA` MUST be namespaces in the compile-time code space (L9367); `Type.SizeOf<T>()` MUST return `T`'s memory size (L9368); `Type` return values MAY enter runtime (L9369); `Compiler`/`NOLAA` return types MUST be the empty type (L9370); `NOLAA` parameters MUST designate heap objects trackable by NOLAA (L9371).
- Example (L9377–L9391): `X`'s final value becomes a runtime constant; runtime `X = 3;` illegal; `local bad: CompileTimeType;` illegal.

### 6.8 §11.8 编译器 API (L9393–L9404)

- The compiler API is the set of `linkable` functions the compiler provides in the compile-time code space (L9395); full signatures in §15.3 (L9397).
- The compiler MAY define APIs outside this standard in the compile-time library (L9401); extension APIs MUST NOT override standard names or change their semantics (L9402); MUST be declared in the compiler's documentation (L9403); a program using extension APIs MUST NOT be considered portable (L9404).

### 6.9 §12.1 内存分配 (L9408–L9503)

Storage durations table (L9412–L9416):

| Duration | Initiated by | Lifetime |
| --- | --- | --- |
| 作用域存储期 (scope) | `local` (§5.2) | determined by the current scope |
| 自动堆存储期 (automatic heap) | `heap` (§5.4) | determined by §12.2 (NOLAA) |
| 手动堆存储期 (manual heap) | `manual heap` (§5.4) | determined by the developer |

- DEFINES memory allocation as the act of obtaining memory space for an instance (L9420); storage duration as the time from allocation to release (L9421); the allocation mode MUST be specified **only** by the identifier definition (L9422); allocation MUST NOT be a component of an expression (L9423); allocation MUST NOT appear in types (L9424); NovaSharp MUST NOT specify a default allocation mode for any type (L9425).
- Types: allocation mode MUST be independent of type (L9429); value types (§3.3) and reference types (§3.4) MUST both be usable with any duration in the table (L9430); when the identifier's type is a reference type, the identifier MUST store a reference to that storage (L9431).
- Construction: when the instance's type declares a constructor, the instance MUST be constructed via a construction expression (§4.6) (L9435); when the instance's type is a variant **or** union, the instance MUST be constructed via a member construction expression (§4.7) (L9436); neither of the two MUST specify the allocation mode (L9437); when either appears on the right of a definition or assignment, the instance MUST be constructed into the target storage of that definition or assignment (L9438).
- Failure: allocation failure MUST produce the `AllocateFailedException` runtime exception (§5.10.1) (L9442).
- Values: all values not bound by an identifier definition MUST be given storage by the compiler (L9446); such allocation MUST NOT produce dangerous behaviour (L9447); its lifetime MUST be as short as possible (L9448).
- Uninitialised storage: DEFINES uninitialised storage as storage not yet written (§5.2, §3.8.1, §5.4) (L9452); storage that may be read before assignment MUST support `uninitialized` as its initial value (§2.7.12) (L9453); storage not initialised with `uninitialized` MAY only be read after assignment (L9454); reading uninitialised storage MUST yield the uninitialised value (§2.7.12) (L9455); the compiler MUST NOT assume a value for uninitialised storage (L9456); when the type is a function type and it is called before assignment, the compiler MUST warn (L9457); when the type is a pointer type (§3.6.2) and it is dereferenced before assignment, the compiler MUST warn (L9458); when the type is a reference type (§3.4) and it is used for member access before assignment, the compiler MUST warn (L9459).
- Escape: a reference to a scope-duration instance MUST NOT escape that instance's scope, except when the reference is a return value and the instance has been constructed into caller-provided storage (§5.10.2) (L9463–L9464). Escape paths are (L9465–L9469): a return value (§5.10.2); a write to global or heap storage (§5.4); capture by a function literal (§2.7.9); being `throw`n (§5.10.3).
- Release: scope-duration instances MUST be released at scope end (L9473); automatic-heap instances MUST be released as decided by the object-lifetime analysis algorithm (§12.2) (L9474); manual-heap instances MUST NOT be governed by that analysis (§12.2) (L9475); the analysis's insertion MUST be performed through `dispose` statements (§5.8) (L9476); automatic- and manual-heap instances MAY be explicitly released with `dispose` (§5.8) (L9477); scope-duration instances MUST NOT be released with `dispose` (§5.8) (L9478).
- Example (L9482–L9503): `local cat`, `heap heapCat`, `manual heap rawCat` are the three legal durations for the same `Cat` type.

### 6.10 §12.2 NOLAA (L9505–L9619)

Responsibilities table (L9509–L9514):

| Responsibility | Objects | Result |
| --- | --- | --- |
| Double-Dispose check | all heap allocation units | Warning |
| Early-Dispose check | all heap allocation units | Warning |
| Never-Disposed check | non-`manual` allocation units | Warning |
| insert `dispose` | non-`manual` allocation units | inserted at their safe point |

- DEFINES: NOLAA as the compile-time heap allocation-unit lifetime analysis tool (L9518); allocation unit as the heap storage obtained by one `heap` (L9519); alias as another identifier pointing at the same allocation unit (L9520); demander as a program point **or** identifier that needs the allocation unit to exist, including aliases (L9521–L9522); existence demand as the fact that a demander needs the unit to exist (L9523); safe point as a node from which, along **all** control-flow paths, no demander occurs again (L9524); dispose point as the location where a `dispose` is inserted (L9525).
- NOLAA MUST run at compile time (L9526); its analysis scope MUST be the whole program, not limited to a scope or function (L9527); MUST NOT govern `manual heap` units (L9528); MUST NOT govern scope-duration instances (§5.2) (L9529); MUST NOT govern compile-time execution allocations (§11.7) (L9530).
- Insertion: NOLAA MUST find the safe point of every unit without `manual` and insert `dispose` there (§5.8) (L9534); NOLAA MUST be conservative — when it cannot prove a node is a safe point it MUST NOT insert (L9535); NOLAA is FORBIDDEN to insert at a non-safe point (L9536); when no safe point exists it MUST NOT insert (L9537); when the unit has no reachable identifier it MUST NOT insert (§5.8) (L9538); when the unit already has an explicit `dispose` it MUST NOT insert (L9539).
- External calls: DEFINES abandoning as NOLAA stopping safe-point reasoning for a unit (L9543); on reaching a call to a `linkable` function NOLAA MUST abandon (§5.5) (L9544); that abandonment MUST NOT insert a dispose (L9545); when it happens the unit is Never-Disposed (L9546).
- API: `NOLAA.Abandon(x)` MUST stop analysis of the heap object `x` points to (L9550); after `Abandon` no dispose may be inserted for that object (L9551); no Never-Disposed warning (L9552). `NOLAA.Bind(required, requiredBy)` MUST add an existence demand between the object `requiredBy` points to and the object `required` points to (L9553); while `requiredBy` exists, `required` MUST NOT be released (L9554); the demand MUST participate in strongly-connected-component determination (L9555). `NOLAA.AssumeNoDemandNext(x)` MUST affect only the single statement immediately following it (L9556); that statement MUST NOT create a new existence demand on the object `x` points to (L9557); when that statement is a `linkable` call, NOLAA MUST NOT abandon because of that call (L9558); normal analysis MUST resume after that statement (L9559). NOLAA API parameters MUST point to NOLAA-trackable heap objects (L9560); if not, the compiler MUST report an error (L9561).
- Checks: NOLAA MUST check Double-Dispose and warn (L9565–L9566, DEFINES destroying the same unit twice); MUST check Early-Dispose and warn (L9567–L9568, DEFINES destroying a unit at a non-safe point; Use-After-Dispose — a unit still having demanders after destruction — is an Early-Dispose) (L9569); MUST check Never-Disposed and warn (L9570–L9571, DEFINES a unit never destroyed), and this check MUST NOT apply when the unit has been `NOLAA.Abandon`ed (L9572); the warnings MUST NOT block compilation (L9573); they MUST identify the unit's identifier and the program location (L9574).
- Cycles: NOLAA MUST treat mutually-requiring units as one analysis unit (L9578); that is a strongly-connected component of the demand graph (L9579); members of one analysis unit MUST be released at the same safe point (L9580).
- Destruction: an inserted dispose MUST trigger its target's destructor (§3.7.1, §3.7.2) (L9584); a destructor MUST NOT depend on the exact moment destruction happens (L9585).
- Example (L9589–L9619): `token` gets its dispose after last use; `Global = shared; dispose shared;` yields an Early-Dispose warning; double `dispose duplicated` yields a Double-Dispose warning.

### 6.11 §13.1 NovaSharp ABI (L9623–L9636)

- DEFINES the NovaSharp ABI as the interface contract by which NovaSharp artifacts call each other at the binary level (L9625), composed of calling convention, memory layout and name mangling (L9629).
- Calling convention: see §15.2 (L9630) — i.e. `enumeration CallingConvention: integer(4)` with values 0–7. Basic types: §3.2, §3.6.2 (L9631). Memory layout: §3.7.2 (structure), §3.7.1 (class), §3.7.6 (union), §3.7.5 (variant), §3.7.4 (enumeration), §3.8.1 (field), §3.6.1 (array) (L9632). Alignment: §9.14 `aligned(N)` (L9633). Mangling: §13.2 (L9634). External symbol names: §5.5 (L9635). Size optimization: §14.3.2 (L9636).

### 6.12 §13.2 名称修饰 (L9638–L9675)

- DEFINES name mangling as the rules encoding NovaSharp names into binary symbol names (L9640).
- A symbol name consists, in fixed order, of: scope path, declaration name, hash of the parameter-type list (L9644).
- A method's `this` type MUST be counted first in the parameter-type list (§8.4) (L9645).
- Generic arguments MUST be counted as the monomorphized type (§10.2) (L9646).
- The symbol name MUST NOT contain the return type (L9647), MUST NOT contain the calling convention (L9648).
- Name components MUST NOT contain whitespace; when a type's spelling may contain whitespace it MUST be omitted (L9649).
- The symbol name MUST be uniquely determined by those three components (L9650).
- Form: the symbol name MUST be written as `N`, then the encoded scope path, then the encoded declaration name, then the hash (L9654).
- Scope path and declaration name MUST each be written as "decimal length + escaped text" (L9655): escaped text contains only `A-Za-z0-9` (L9656); every other character MUST be written as `_` plus two lowercase hex digits (L9657); `_` MUST be written as `_5f` (L9658); the length is the byte count of the escaped text (L9659).
- Hash MUST be FNV-1a 64-bit, written as 16 lowercase hex digits (L9660): offset basis `0xcbf29ce484222325`, prime `0x100000001b3` (L9661); its input is the byte sequence formed by concatenating each parameter type's encoding in order (L9662); each parameter type's encoding is "decimal length + escaped text" with the same escaping (L9663).
- Components MUST be concatenated directly with no other separators (L9664).
- Different parameter-type lists MAY share a hash, but it MUST be a low-probability event (L9665).
- Worked example (L9667–L9669): scope path `Main`, declaration name `Compute`, parameter-type list `integer(4)` ⇒ `N4Main7Compute2bf5d948506c5341`. This is checkable and **verified**: `integer(4)` escapes to `integer_284_29` (length 14) ⇒ `14integer_284_29`; FNV-1a-64 of those bytes = `2bf5d948506c5341`; prefixing `N` + `4Main` + `7Compute` reproduces the spec string exactly. (The length prefixes **are** part of the hashed bytes.)
- External symbols: when a definition is `linkable`, its symbol name MUST be the exposed symbol name of that definition (§5.5) (L9673); when `linkable` omits the parameter, the symbol name MUST be the name (§2.4) (L9674); when `linkable` carries a string parameter, the symbol name MUST be that string's content (L9675).

### 6.13 §14.1 主函数 (L9679–L9717)

- DEFINES the main function as the function constant named `Main` (§5.3) (L9685); an executable product MUST have exactly one main function (L9686).
- The main function MUST be called only by the Launcher (§14.2) (L9690); its return type MAY be omitted (§14.2) (L9691).
- Examples: `fixed Main(): integer(4) { return 0; }` — return value handed as-is to the entry point (L9697–L9700); `fixed Main(handle: pointer, table: pointer): integer(4)` — platform arguments, interpreted by the callee itself (L9703–L9706); `fixed Main() { return; }` — empty return type, the entry point receives no value (L9709–L9712); two `Main`s illegal (L9715–L9716).

### 6.14 §14.2 启动流程 (L9719–L9795)

- DEFINES startup as the execution order from the entry point to termination (L9725); entry point as the symbol independent of the language standard, selected by the build and linker (L9726); Launcher as a function generated by the compiler **and** called by the entry point (L9727); compile-time initial value as an initial value the compiler can evaluate at compile time (§11.1) (L9728); runtime initialisation as initialisation of module-level or static storage whose value is not compile-time determinable (L9729).
- Order (L9733–L9737): (1) the entry point calls the Launcher; (2) the Launcher executes all static constructors (§9.15) and all runtime initialisation so the program reaches the initialised state; (3) the Launcher calls the main function; (4) when the main function's return type is not the empty type, the Launcher MUST hand the return value as-is to the entry point, which terminates the program with it; when the return type is empty the Launcher MUST NOT pass a value.
- Sections (L9741–L9746): when a module-level or static storage initial value is compile-time determinable, the compiler MUST emit it into the executable image (L9741) — non-function constants' initial values MUST go into a read-only section (L9742), variables' and static fields' initial values MUST go into a writable section (L9743), and those initial values MUST take effect before the entry point is called (L9744); when not determinable, it MUST be initialised by the Launcher at runtime (L9745); after runtime initialisation of a constant completes, its storage MUST NOT be written again (L9746).
- Initial state (L9750–L9753): the entry point's initial state is provided by the build and linker and MUST match the target platform (L9750); the Launcher MUST guarantee that every function it calls sees the same initial state as when the entry point was called (L9751), and that state MUST be reproduced in the form of argument values (L9752); how the callee interprets that state MUST be decided by the callee itself (L9753).
- Initialisation (L9757–L9762): static constructors MUST complete before all user code executes (L9757); runtime initialisation MUST complete before all user code executes (L9758); the order among these initialisations MUST be determined by the compiler (L9759) and MUST stay consistent within one program (L9760); the compiler MUST optimise that order so all determinable dependencies are satisfied, i.e. a depended-on initialisation MUST precede its dependents (L9761); when determinable dependencies form a cycle the compiler MUST report an error (L9762).
- Responsibility split (L9766–L9769): entry-point selection is the build and linker's (L9766); the entry point's initial state being platform-consistent is the build and linker's (L9767); Launcher generation is the compiler's (L9768); passing the return value to the entry point is the Launcher's (L9769).
- Note (L9771–L9774): the entry point is outside the language boundary; it is called from outside the language, its initial state is guaranteed to match the platform by the build and linker, and the program interprets that state itself.
- Example (L9778–L9794): `Main` returns `0`; startup performs the 4 steps; `module A { local X: integer(4) = B.Y; }` + `module B { local Y: integer(4) = A.X; }` is illegal (cycle in initialisation dependencies).

### 6.15 §14.3.1 编译优化总则 (L9799–L9881)

- Shared constraints (L9805–L9809): optimisation MUST NOT change syntax (L9805); MUST NOT change a structure's field order, size, offset or alignment (L9806); MUST NOT change observable behaviour (L9807); MUST NOT break `linkable` symbols, externally visible ABI or required metadata (L9808); optimisation MAY assume whole-program compilation (L9809).
- Whole-program dead-code elimination (L9818–L9820): MAY delete runtime-unreachable declarations, members, generic instances, interface implementations and Built-In definitions (L9818); MUST keep `linkable` exported symbols (L9819); MUST keep metadata needed for exception interception, type testing and interface conversion (L9820).
- Monomorphization dedup and identical-code folding (L9824–L9827): MAY merge generic instances and compile-time instances with identical machine code (L9824); MAY merge function bodies with identical machine code (L9825); MUST keep each generic instance's type identity and static fields (L9826); MUST NOT merge the function addresses corresponding to `linkable` exported symbols (L9827).
- Metadata minimisation (L9831–L9834): MAY delete runtime-unneeded type names (L9831); MAY merge identical type descriptors and interface tables (L9832); MAY delete unused vtable slots and interface-table entries (L9833); MUST keep type information needed by `is` expressions, interface conversions and `trap` (L9834).
- Captureless function values (L9838–L9840): a captureless function value MAY be represented as a function pointer (L9838); a capturing function value MAY share function code and keep only the environment data (L9839); the compiler MUST preserve argument passing, capture semantics and call results (L9840).
- Whole-program devirtualisation (L9844–L9847): MAY replace virtual and interface calls with determinable targets by direct calls (L9844); MAY delete unreachable vtable slots and interface-table entries (L9845); MUST keep dynamic dispatch results unchanged (L9846); MUST NOT remove externally visible `linkable` symbols (L9847).
- Compile-time residual cleanup (L9851–L9853): MAY fold constant expressions at compile time (L9851); MAY delete compile-time residual declarations not referenced by runtime (L9852); MUST keep terminal products needed by runtime (L9853).
- Representation boundaries (L9857–L9861): the language definition of the Built-In Library MUST be preserved; its unreferenced code and data MAY be omitted from the artifact (L9857); a class instance's type information and the vtable body MUST be preserved — only names, unused slots and unused entries MAY be deleted (L9858); an interface table is dispatch metadata and MUST NOT be treated as the runtime entity of an interface member (L9859); function types remain reference types; a function pointer is only an internal or ABI representation (L9860); index-access bounds checking per §4.4.3, and this section MUST NOT change out-of-bounds semantics (L9861).
- Common budgets (L9863–L9881): artifact size — total growth MUST be < 2% (L9867), per-feature-dimension growth MUST be < 5% (L9868). Runtime cost — a "significant regression" is (L9872–L9879): corpus total runtime geometric mean ≥ 3%; any representative workload runtime ≥ 10%; p95 latency ≥ 10%; peak memory ≥ 5%; allocation count or allocated bytes ≥ 10%; startup time ≥ 10%. Shared optimisations MUST NOT exceed those budgets; the size-specific and speed-specific sections MAY add stricter conditions (L9881).

### 6.16 §14.3.2 体积优化 (L9883–L9899)

- Literal pooling (L9888–L9891): identical string, array and constant literals MAY share read-only data (L9890); the compiler MUST preserve value semantics — assignment and argument passing still copy by value (L9891).
- Pass conditions (L9893–L9897): corpus total net decrease MUST be ≥ 2% (L9895); the corresponding feature dimension's net decrease MUST be ≥ 10% (L9896); no metric may exceed the size and runtime budgets of §14.3.1 (L9897).
- Shared optimisations are defined in §14.3.1 and are not repeated (L9899).

### 6.17 §14.3.3 速率优化 (L9901–L9951)

- `trap` fast path (L9906–L9909): MAY optimise the exception-free path of `trap` (L9908); MUST NOT change `catch` matching, `finally` execution, or termination behaviour when uncaught (L9909).
- Small-function inlining (L9911–L9916): MAY inline hot and small functions (L9913); total artifact size growth MUST be < 2% (L9914); the target workload speedup MUST be ≥ 5% (L9915); any workload regression MUST be < 3% (L9916).
- Hot-path specialisation (L9918–L9922): MAY specialise type-stable hot call sites (L9919); total artifact size growth MUST be < 2% (L9920); the target workload speedup MUST be ≥ 5% (L9921).
- Bounds checks (L9924–L9927): bounds-check elimination MUST follow §4.4.3 (L9926); this page MUST NOT add optimisations that change out-of-bounds semantics (L9927).
- Not adopted (L9929–L9936): reference-type generic sharing; interface-table globalisation or interface fat pointers; structure layout adjustment; NOLAA batch release; aggressive inlining or loop unrolling; trading increased peak memory for speed.
- Pass conditions (L9938–L9949): overall runtime (geometric mean) speedup ≥ 5%; target workload speedup ≥ 10%; any representative workload regression < 3%; p95 latency regression < 5%; peak memory increase < 5%; startup time regression < 5%. When any condition is unmet, the optimisation MUST NOT be enabled (L9949).
- Shared optimisations are defined in §14.3.1 and are not repeated (L9951).

## 7. Open questions

Each item: what the spec says (line anchors), why it is a gap/inconsistency, and the interpretation this project should adopt. Interpretations are proposals, not spec text.

**Q1 — §15.1 is not a complete grammar, and 40 productions are elided with `…`.**
§15.1 calls itself "NovaSharp 全部语法点的清单" (L9957) but uses `…` in 46 productions (e.g. L9964, L9965, L9967, L9968, L9971, L9972, L9975, L9992, L9999–L10002, L10005, L10007–L10008, L10010, L10017–L10018, L10019–L10028, L10046, L10048–L10050, L10064–L10066, L10069, L10072, L10078, L10082, L10084, L10091–L10096, L10104–L10105, L10118, L10123–L10124, L10128) and omits `CompoundSetStatement`, `AssignmentTarget`, `TraverseTarget`, `Range`, `MatchCase`/`Pattern`, `TrapBody`/`CatchCondition`, `CallingConventionExpression`, `TargetBinding`, `Element`/`RepeatElement`/`RangeElement` entirely. Proposed: treat the governing sections as normative for productions, use §15.1 as an index, and use “Completed productions” above as the parser's production set. Also treat `ExtensionDeclaration` (L9999, L10077) as one production, listed twice.

**Q2 — The compile-time constant-parameter convention `#` is required for overload identity but forbidden in the mangled name.**
§11.5 L9183–L9184: `#` MUST participate in function-type equality and in the overload signature. §13.2 L9644–L9648 and L9662–L9663: the symbol name is scope path + declaration name + hash of parameter *types* only, and MUST NOT contain the return type or the calling convention. So `f(x: integer(4))` and `f(#x: integer(4))` are distinct overloads but would mangle identically (`N…2…<same hash>`), contradicting L9650 ("MUST be uniquely determined by the three components"). Proposed: include a per-parameter convention marker (e.g. a leading `#` byte or a distinct escape) in the hash input while keeping the printed name shape, **or** forbid such overload pairs in sema. This needs a spec erratum; pick the hash-input marker so existing names stay stable for non-`#` parameters. `&` parameters have the same problem (L10094 grammar vs L9663 hash input).

**Q3 — Scope-path encoding for nested scopes is unspecified, and the hash of an empty parameter list is unspecified.**
§13.2 L9655 says "作用域路径与声明名必须各写为「十进制长度 + 转义文本」" — singular length+text — while a scope path such as `Outer.Inner` has multiple components, and the separator `.` is not in `A-Za-z0-9` (so it would escape to `_2e`, making the path indistinguishable from a declaration name containing a dot). Also `Type.SizeOf<T>()` has an empty parameter list, so the hash input is the empty byte sequence, which is not stated. Proposed: encode the scope path as one length+text of the joined path with `.` escaped as `_2e` (keeps the worked example intact), and define the empty-list hash as FNV-1a-64 of zero bytes, i.e. the offset basis `cbf29ce484222325`. Record the choice in `docs/` as an ABI decision.

**Q4 — `CallingConvention` values have no lowering table.**
§13.1 L9630 delegates the calling convention to §15.2, but §15.2 only declares `enumeration CallingConvention: integer(4)` with values 0–7 (L10169–L10187) and doc glosses; no rule maps a value to a machine-level convention, no rule says which value a function type without `; CallingConventionExpression` gets, and no rule says whether non-default conventions are permitted on non-`linkable` functions. Proposed: `PlatformDefault = 0` means the target's default C convention; per-value targets: 1 → C (`ccc`), 2 → `x86_stdcall`, 3 → `x86_fastcall`, 4 → `x86_64_sysv`, 5 → `x86_thiscall`, 6 → `x86_vectorcall`, 7 → `win64`; a function type with no convention expression is `PlatformDefault`; mismatch on a `linkable` boundary is an error.

**Q5 — The `Main`/Launcher call contract leaves the signature and argument shape open.**
§14.1 L9685–L9691 says `Main` is the function constant named `Main`, has exactly-one cardinality, is called only by the Launcher, and its return type MAY be omitted. §14.2 L9751–L9753 requires the Launcher to reproduce the entry point's initial state "以实参值的形式" and lets the callee interpret it; the example at L9703 shows `Main(handle: pointer, table: pointer)`. Unspecified: how the Launcher knows `Main`'s parameter list; whether `Main` may be generic or take compile-time constant parameters (which would make its type non-runtime); whether an omitted return type means the empty type (the example's comment says 返回值为空类型, L9708); how many/which arguments are passed when `Main` has a different arity than the platform entry point; and what happens with `&`/`#`/default parameters. Proposed: require `Main` to be non-generic, with at most two `pointer` parameters and a return type of either empty or a basic integer type matching the platform's exit-status width; omitted return type = empty type; extra entry-point arguments are dropped; missing ones are passed as null/zero; document the mismatch cases as errors.

**Q6 — §11.7 calls `Type`/`Compiler`/`NOLAA` "namespaces" while §15.3 declares them `module`, and `Type.SizeOf<T>()` has a width contradiction.**
§11.7 L9367: they MUST be "编译期代码空间中的命名空间". §15.3 L10369/L10377/L10406 spell them `module`. There is no `namespace` keyword in §15.1's module list (only `ModuleDeclaration`), so "命名空间" is presumably descriptive. Separately: §15.3 L10373 declares `SizeOf<T>(): cardinal(8)` and its doc comment says the value's width equals the target pointer width (L10372), which conflicts on a 32-bit target (a pointer-sized result would be `cardinal(4)`, but the declared return type is fixed at `cardinal(8)`), and §11.7 L9368 only says "`T` 的内存大小". Proposed: `module` == namespace; `SizeOf` returns a `cardinal(8)` value whose radix is the pointer width (i.e. value zero-extended on 32-bit targets); treat the doc comment as descriptive of the intended use, not of the radix.

**Q7 — The `message[length: cardinal(4)]: cardinal(1)` parameter form is not derivable from the EBNF.**
§15.3 L10382/L10387/L10392/L10402 use an array-typed parameter whose dimension carries a **named binding** `length: cardinal(4)`, so each function gains an implicit second parameter `length` that callers do not spell. §3.6.1's `ArrayTypeConstructor = "array(" Expression { "," Expression } ")"` (§15.1 L10004) admits only expressions, and `TargetBinding = Name ":" Type` appears only in `ArrayTarget` for extension targets (§3.8.3). Unspecified: whether `length` is a real parameter, who computes it (compiler or caller), whether it is in declaration order after `message`, and whether it participates in mangling/overloads. Proposed: desugar to `LogNote(message: array(length) cardinal(1), length: cardinal(4))` with the compiler passing `length` = element count (array type is `array(length)` of `cardinal(1)`, so bytes == elements); the implicit parameter participates in mangling after the array parameter; callers write only the array argument.

**Q8 — Default parameter values must be compile-time constants, but the Built-In Library uses `Optional<Exception>.Null` as a default.**
§11.7 L9312–L9316 enumerates the only sources of compile-time constants (literal, generic parameter, compile-time constant parameter, a value produced in `#{}`, a `fixed` constant with compile-time-evaluable initialiser). `Optional<Exception>.Null` (L10200, L10229, L10244) is a payload-less variant member construction — none of those five sources. Either the list is not exhaustive, or variant member construction of a payload-less member is a compile-time constant. Proposed: add "member construction of a variant/union member with no payload" and "construction of an empty-type value" to the constant sources; require default parameter expressions to be compile-time constants generally.

**Q9 — The Built-In Library is far smaller than the rest of the spec implies, and some referenced types have no definition.**
§15.2 claims to be "强制附带的全部定义" (L10159) yet ships only 13 definitions: `Object`, `CallingConvention`, `Exception` + 5 derived exceptions, `IIterator<T>`, `IIterable<T>`, `IAllocator`, `Optional<T>`, `Result<TValue; TAlternative>`. There is no string type (though string literals are normative, L9964), no array type with an `IIterable` implementation (yet `for` traverses arrays/iterables, L10082), no `IAllocator` implementation, no `Object`-level members, no metadata type for `is`/`Type`, no threading/IO. Also `Optional<T>` has no implicit conversion from `T`, so `Optional<Exception>.Null` shows the only construction idiom. Proposed: treat §15.2 as the *mandatory minimum* that the runtime must ship; other built-ins are implementation-defined but must not conflict; keep the 13 definitions byte-identical in the compiler's built-in module; document the string/array runtime representation separately (it is not in the spec range).

**Q10 — `Construct` is declared as an extension method but §12.1 speaks of "构造函数" and the `Cat()` construction expression.**
§12.1 L9435 requires construction via a construction expression when the type declares a constructor; L9436 requires member construction for variants/unions. §15.2 declares `Construct` as `public fixed Construct(this: …)` inside `extension` blocks (L10200, L10214, L10229, L10244, L10258, L10272) — i.e. an ordinary fixed function whose `this` is the instance, not a keyword-level constructor. §12.1's example constructs `Cat()` (L9493) although the shown `Cat` (L9484–L9487) declares no `Construct`. Unspecified: is `Construct` *the* constructor convention, what is `Cat()` when no `Construct` exists (zero-initialisation? implicit default constructor?), and can a constructor be overloaded/`#`-parameterised. Proposed: any `extension` function named `Construct` with a `this` parameter is a constructor; `T()` with no matching `Construct` zero-initialises the instance; constructors participate in overload resolution like ordinary functions.

**Q11 — Compile-time ordering has a bootstrap cycle between `#if` selection and "all declarations name-resolved".**
§11.2 L8983: top-level `#{}` MUST execute after **all selected** declarations complete name resolution. §11.2 L9011–L9012: top-level `#if` interleaves with top-level `#{}` by source order and its condition MUST be evaluated on bindings evaluated before it. Selection of `#if` branches is thus needed before name resolution of "all selected declarations", while `#{}` results are needed to evaluate later `#if` conditions. Proposed: a streaming model — process top-level items in file/source order; for each item, (a) `#if` ⇒ evaluate its condition now, descend into the selected branch (recursively), (b) declaration ⇒ name-resolve and collect it now, (c) `#{}`/`#()` ⇒ execute now; "all selected declarations" is then read as "all declarations selected up to this point". Anything else (e.g. two passes) contradicts L9011.

**Q12 — "Incomplete control flow" in `#{}` is undefined.**
§11.4 L9127–L9128 requires preserving incomplete control flow "以最简形式" and errors when preservation fails, but never defines what "incomplete" means (fallthrough without a terminal statement? `break` out of a `#{}` nested in a loop that the `#{}` itself does not own?). Combined with L9123 (`return`/`break`/`continue` bind to the enclosing function/loop) and L9114 (`#{}` is not a scope), the case "`#{}` ends in `return` inside a function that must still return a runtime value" is unclear. Proposed: "incomplete" = a path reaching the end of the `#{}` that leaves the enclosing function/loop in a state needing a runtime jump; represent it as the minimal branch + jump into the enclosing flow; a `return` inside `#{}` whose value is compile-time constant folds to a constant return (as in the `Apply` example, L9203–L9208).

**Q13 — NOLAA's safe-point definition vs. loops, aliasing through `Bind`, and `Abandon` inside an SCC.**
§12.2 L9524 defines a safe point as a node from which **all** paths have no further demander; in a loop whose next iteration re-reads the unit there is no such node ⇒ no insert (L9537) ⇒ Never-Disposed warning (which for a non-`manual` unit is noise). Unspecified: whether a "demander" includes merely passing the unit's identifier to a function (L9544 answers only for `linkable` calls), whether `Bind`-created demands expire with the `requiredBy` unit's own disposal (L9554 says "存在期间" but not how that is decided in a cycle), and whether `Abandon(x)` for one SCC member releases the whole SCC from analysis or only `x`. Proposed: demanders are reads of the identifier, member accesses through it, `&` of it, return/throw/capture of it, and writes of it into global/heap storage; `Bind` demands are edges in the SCC graph and are dropped when the `requiredBy` node is itself disposed at the SCC's common safe point; `Abandon(x)` removes `x`'s outgoing reasoning only, so an SCC containing an abandoned member gets no insertion and no Never-Disposed warning for that member but still may warn for other members.

**Q14 — §14.3.1's regression table reads as both a definition and a limit.**
L9872–L9879 tables "指标 / 显著回归" with thresholds and L9881 says shared optimisations MUST NOT exceed those budgets. It is unclear whether the numbers define "significant regression" (i.e. a regression is *reportable* if it reaches the threshold) or are hard limits (a regression must stay below them). Proposed: hard limits on the optimizer's enabled output (any exceeded ⇒ the optimisation must not ship), matching §14.3.3 L9949's explicit rule.

**Q15 — §14.2's "initialised state" versus the Launcher's own preconditions.**
§14.2 L9734 has the Launcher execute all static constructors and runtime initialisation, then L9751 requires every called function to see the entry point's initial state "以实参值的形式复现". If a static constructor or runtime initialiser is itself a function, does it also receive the reproduced initial state (thread pointer, stack bounds, argc/argv)? Also L9746 forbids writing a constant's storage after its runtime initialisation, which conflicts with `fixed` values assigned inside `Construct`-style init if they are emitted into read-only storage (L9742). Proposed: only `Main` (and any `linkable` boundary the platform requires) receives the reproduced state; initialisers are plain internal functions with an ordinary empty-argument signature; a `fixed` whose initialiser is not compile-time determinable is placed in writable storage and frozen after init, and only compile-time-determinable non-function constants go to read-only storage.

## 8. LLVM 18 codegen implications

### 8.1 Symbol names and the ABI surface

- Emit every non-`linkable` declaration with a mangled symbol (§13.2 L9654) or keep it `internal`; either way the mangled name is the only name the ABI fixes. `linkable` definitions MUST be emitted with their exposed symbol (§5.5 L9673): the declared name when no `(...)` parameter is given (L9674), or the literal string content (L9675). Those symbols MUST be preserved through optimisation (§14.3.1 L9819, L9827, L9847) — annotate with `used`/`externally_initialized` as appropriate and never let identical-code folding merge their addresses (L9827).
- Mangling implementation recipe: for each parameter type in order (with a method's `this` type first, L9645; generic arguments instantiated, L9646), render the type's canonical spelling, then emit `len(escaped) + escaped` with `A-Za-z0-9` kept and everything else `_xx` lowercase hex (`_` → `_5f`, L9657–L9659), concatenate those encodings in order without separators (L9664), hash with FNV-1a-64 using offset `0xcbf29ce484222325` and prime `0x100000001b3` (L9661), print the hash as 16 lowercase hex digits (L9660). Final name: `N` + `len(escaped scope path) + escaped` + `len(escaped decl name) + escaped` + hash (L9654–L9664). Verified against the spec's example quoted under “§13.2 名称修饰” above.
- Canonical type spelling used for mangling must be the post-normalisation form: whitespace removed (L9649), array dimensions rendered from the `array(...)` arguments, `pointer` as the literal keyword (§3.6.2), `function(...)` per §3.6.3 including `ParameterList`'s `wildcard` form, `&` and `#` handled per Q2.
- `CallingConvention` (L10169–L10187): attach the lowered LLVM calling convention on the `FunctionType`; `PlatformDefault = 0` must follow the target triple's default. Because the mangled name must not encode the convention (L9648), two functions with the same types and different conventions would collide — reject such pairs in sema until Q4/Q2 are resolved.
- Layout is referenced but not specified in this range: field order/size/offset/alignment are fixed (§3.7.1, §3.7.2, §3.7.6, §3.7.5, §3.7.4, §3.8.1, §3.6.1, L9632) and `aligned(N)` (§9.14, L9633); `aligned` needs to be honoured to avoid violating §14.3.1 L9806 ("MUST NOT change field order, size, offset or alignment"). Value types are passed per their layout; reference types are pointers to storage (§12.1 L9431); `pointer` is an opaque `ptr`-class value; arrays are value types with a compile-time length.
- Struct/class/union/variant/enumeration layout algorithms must be derived from §3.7.x/§3.8.1 (outside this range) and then frozen — treat the layout as part of the ABI (§13.1 L9629).

### 8.2 Startup, initialisation, and `Main`

- Generate one Launcher per executable product (§14.2 L9727, L9768). The entry point itself is chosen by the build/link (L9766) and is outside the language (§14.2 L9771–L9774). The Launcher must have the same signature as the entry point's inner call boundary: on a typical ELF target `Main() -> integer(4)` means the Launcher returns `i32` and the entry point terminates with it; an empty `Main` return means the Launcher/entry point passes no value (L9736–L9737).
- Launcher body order (L9733–L9737): (1) static constructors, (2) module-level/static runtime initialisation, (3) call `Main`, (4) `ret` the result as-is for a non-empty return type, `ret void`/`ret i32 0` per the platform otherwise. Both (1) and (2) MUST complete before any user code (L9757–L9758).
- Initialisation ordering: build a dependency graph over module-level/static initialisers; topologically sort (L9761); the order MUST be stable/consistent (L9760); a cycle is a compile error (L9762) — emit it at the declaration site of the initialiser, naming the cycle. The `A.X`/`B.Y` example (L9791–L9794) is the expected diagnostic.
- Constant placement: compile-time-determinable initial values must be baked into the image before the entry point (L9741, L9744) — non-function constants into a read-only section (`llvm.global constant`, `.rodata`), variables/static fields into a writable section (`llvm.global`, `.data`/`.bss` for zero) (L9742–L9743). Non-determinable initial values are written by the Launcher at runtime (L9745); a constant's storage must not be written after that (L9746) — mark such globals `constant` only when truly never written, otherwise keep writable and rely on sema.
- Reproduce the entry point's initial state as argument values (L9750–L9753): thread the platform arguments through the Launcher to the callee that needs them; the callee interprets them (L9753). Do not model the state as hidden globals if a function is declared to receive it as parameters (the L9703 example gives `Main(handle: pointer, table: pointer)`).
- `Main` lookup: exactly one `Main` per executable product (L9686) is a driver-level link-time check; two `Main`s is an error (L9715–L9716). `Main` is a `fixed` function constant (§5.3), so a `linkable`-named `Main` should still be rejected/aliased per Q5.

### 8.3 Allocation and NOLAA in IR

- Three storage durations (L9412–L9416): `local` → `alloca` in the entry block (scope-lifetime, released at scope end, L9473, and MUST NOT be `dispose`d, L9478); `heap` → call to `IAllocator.Allocate(size, alignment)` returning `Result<pointer; AllocateFailedException>` (L10307); `manual heap` → same call but the result is excluded from NOLAA (L9528, L9475).
- Allocation-failure handling: a `Result` whose `Alternative` is an `AllocateFailedException` must be turned into a throw of `AllocateFailedException` (L9442) inside the implicit `trap` region at the point of allocation, i.e. an `invoke`/landing-pad path, not a null return.
- NOLAA is a whole-program pass (L9527) run over the merged IR (before codegen decisions that would obscure demand edges). Model it as: demand graph over allocation units (nodes) with demand edges from each demander (identifier/program point, incl. aliases, L9521–L9522); SCC condensation (L9578–L9580); for each non-`manual`, non-`Abandon`ed component, find a node where no demander is reachable on any path (L9524) and split there — insert `dispose` (a call to the destructor, L9584) for every member of the component at that common point (L9580).
- Conservatism is mandatory (L9535–L9538): if any path reaches a demander after the candidate point, do not insert; if no safe point exists, insert nothing; if no reachable identifier exists, insert nothing; if an explicit `dispose` exists, insert nothing. Warnings (L9565–L9574) must carry the unit's identifier and location; never emit an IR-level diagnostic without a source anchor.
- `linkable` calls are analysis barriers: abandon safe-point reasoning for any unit reachable from the call site (L9544–L9546) — a conservative "escapes to external code" edge. `NOLAA.Abandon(x)` (L9550–L9552) suppresses both insertion and the Never-Disposed warning; `NOLAA.Bind(required, requiredBy)` adds a demand edge (L9553–L9555); `NOLAA.AssumeNoDemandNext(x)` suppresses demand edges from exactly one following statement and does not trigger abandonment for a `linkable` call there (L9556–L9559).
- Scope-duration objects must not be handled by NOLAA (L9529); compile-time allocations never reach IR at all (L9355–L9357). No GC, no tracing collector: lifetimes are entirely static — the pass is the only collector.
- Destructor contract: `dispose` triggers the destructor (L9584) and the destructor must not depend on the exact moment (L9585) — so the pass may legally move disposal to the safe point; no ordering guarantees may be relied on by user code.
- Uninitialised storage (L9452–L9459): `uninitialized` must lower to `undef`/`poison` plus `llvm.lifetime`-style scoping only if it cannot be observed; reads of never-assigned storage must not be given a value (L9456). Emit the three required warnings (function call / pointer deref / member access before assignment, L9457–L9459) from sema, not from IR.
- Escape analysis (L9463–L9469): a scope-duration instance may be returned only when it has been constructed into caller-provided storage (sret). Treat sret construction as the sole legal escape for `local`; writes to globals/heap, closure capture and `throw` are escapes and must be rejected for scope storage unless the value is copied.

### 8.4 Compile-time evaluator

- `#(...)`, `#{...}`, `#if` are front-end evaluations, never IR: no runtime replay of compile-time order (L9030) and no runtime access to compile-time bindings (L9031). Fold their results into constants (`llvm::Constant`) at their use sites; `#{ }` terminal products become runtime constants or code (L9132).
- Evaluate per monomorphized instance: function-body `#{}`/`#()` and body `#if` are re-evaluated for each generic/compile-time-constant-argument instance (L9017, L9023, L9275). This forces monomorphization to happen in sema before IR.
- Compile-time values live in a compile-time memory space; all its allocation units MUST be released at the end of compile-time execution (L9355) and must never leak into runtime data (L9356); NOLAA does not apply (L9357).
- Compile-time variables: `local` inside `#{}` only (L9322); assignable only at compile time (L9323); final value may be materialised as a runtime constant (L9324); any runtime write is an error (L9325). Represent as a sema-side constant table, not an IR global, unless referenced by runtime code (then emit a `constant` global initialised from the final value).
- Side effects during compile-time execution are permitted but MUST NOT reach runtime terminal products **and** MUST warn (L9140, L9351) — produce the warning even when the effect is later discarded; keep a diagnostics channel distinct from `Compiler.LogWarning` (the API emits *user-requested* notes/warnings/errors, L10379–L10392).
- `Type.SizeOf<T>()` (L10373) reads the layout engine: `T`'s allocated size including padding, in bytes, as `cardinal(8)` (L9368). It must agree with the layout used by codegen (see 8.1), otherwise ABI and `#()` constants diverge.
- `Compiler.LogNote/LogWarning/LogError/Assert` (L10382–L10402) are compile-time-only: implement as evaluator intrinsics that emit diagnostics; `LogError`/failed `Assert` must fail the compilation; the `message`+`length` pair is `array(length) cardinal(1)` (Q7) — validate the array length at the call site.
- Compile-time `linkable` calls are not runtime calls: they may be evaluated by the interpreter, and their function values MUST NOT enter runtime (L9339, L9366); a `linkable` function's return value MAY (L9345).
- Compile-time constants are an additive monoid of sources (L9312–L9316, Q8): literals, generic parameters, `#` parameters, values produced by `#{}`, compile-time-evaluable `fixed` constants, plus (proposed) payload-less variant/union member construction. An ordinary `local` is never a compile-time constant (L9318), even if its initialiser is.

### 8.5 Optimisation pipeline and flags

- Pass pipeline must respect §14.3.1's shared constraints (L9805–L9809): no struct layout changes (LLVM's `SROA`/`GVN`/`InstCombine` are fine; avoid anything that reorders fields), no observable-behaviour change, keep `linkable`/ABI/metadata. `llvm::GlobalOpt`-style transforms must not drop `linkable` symbols.
- Required to keep: `linkable` exported symbols (L9819, L9827, L9847); type info for `is`, interface conversion and `trap` (L9820, L9834); class type info and vtable body — only names/unused slots/entries may be removed (L9858); interface tables are dispatch metadata, not member entities (L9859).
- Permitted: whole-program DCE of unreachable declarations/members/generic instances/interface impls/built-ins (L9818, with the Built-In *definition* preserved, L9857); identical-code folding of monomorphized instances and bodies while preserving each instance's type identity and static fields (L9824–L9826); metadata merging/minimisation (L9831–L9833); captureless function values as raw function pointers (L9838), capturing ones with shared code + environment (L9839) — note the spec still requires function *types* to be reference types (L9860), so the pointer representation is internal only; whole-program devirtualisation (L9844–L9846).
- Forbidden optimisations (L9929–L9936): reference-type generic sharing; interface-table globalisation or interface fat pointers; structure layout adjustment; NOLAA batch release ("batch free" of several units at once); aggressive inlining or loop unrolling; trading peak memory for speed. Also forbidden by §14.3.1: changing out-of-bounds semantics (L9861), breaking `linkable`/ABI/metadata (L9808).
- Gate any size/speed optimisation on its pass conditions before enabling (L9881, L9949): §14.3.2 needs ≥2% overall and ≥10% per-dimension net size decrease with no budget overrun (L9895–L9897); §14.3.3 needs ≥5% overall geometric-mean speedup, ≥10% target-workload speedup, <3% any-workload regression, <5% p95 regression, <5% peak-memory increase, <5% startup regression (L9942–L9948), and "when any condition is unmet, the optimisation MUST NOT be enabled" (L9949).
- `verbatim` (§9.8, via §15.1 L10121) prohibits compile-time optimisation: do not constant-fold, inline, dedup or devirtualise through annotated code; keep it as an optimisation barrier (`optnone`-like or explicit pass exclusion), and still honour the ABI.
- `trap` fast path (L9908) may mark the no-exception path `cold`-free/normal while `catch` matching, `finally` and uncaught termination stay semantically identical (L9909). Bounds checks: no elimination that changes §4.4.3 semantics (L9926–L9927, L9861).
- Startup-time budget (L9879, L9947) constrains the Launcher: keep static-initialiser work minimal, prefer compile-time-initialised data in the image over runtime initialiser code (L9741), and do not add a runtime pass to reorder initialisers.
- Process-level budget checks are CI artefacts, not per-function decisions: the artifact must not exceed the shared budgets (L9867–L9868, L9874–L9879) and the enabled optimisations must be recorded so §14.3.2/§14.3.3's gates can be re-verified.

## 9. Coverage checklist

- Read in full: L8951–L10425 (one 900-line read at 8951, one at 9851; contiguity verified by the tool's line ranges).
- All 16 fenced blocks in range accounted for: 3 `ebnf` (quoted verbatim under “Verbatim EBNF blocks”), 13 `novasharp` examples (quoted or summarised in the semantic-rules section with anchors; the two large ones — §15.2 at L10161–L10359 and §15.3 at L10367–L10421 — quoted verbatim in the Built-In Library and Compile-Time Library sections), 0 other fences.
- Tables in range: storage durations (L9412), NOLAA responsibilities (L9509), runtime-cost budgets (L9872), §14.3.3 pass conditions (L9940) — all reproduced.
- Numbers that are normative and must become constants/tests: 2% / 5% / 3% / 10% / 5% / 5% / 5% / 5% / 3% (§14.3.1–§14.3.3), 16 hex digits + FNV constants (§13.2), 0–7 enum values (§15.2), `integer(4)` / `cardinal(8)` / `cardinal(4)` / `cardinal(1)` widths in §15.3.
- Not covered by this note (outside the requested range, deferred to the notes for §1–§10): §2 lexical details, §3.4/§3.7 layout, §4.9.26 `??` decision semantics, §5.2/§5.4/§5.5/§5.8/§5.9 exact rules, §8 overload resolution, §9 modifiers, §10 generics.
