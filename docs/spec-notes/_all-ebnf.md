# All EBNF blocks extracted from Draft 1.1 (114 blocks)

## 2.4 标识符  (spec line 329)

```ebnf
Identifier       = Name | MemberAccessExpression .

Name             = ( "a" .. "z" | "A" .. "Z" | "_" ) { "a" .. "z" | "A" .. "Z" | "0" .. "9" | "_" } .
```

## 2.5 标识符定义简写  (spec line 404)

```ebnf
IdentifierDefinitionShorthand = FunctionShorthand | ArrayShorthand .

FunctionShorthand = Name [ GenericParameters ] "(" [ Parameters | "wildcard" ] [ ";" CallingConventionExpression ] ")" [ ":" Type ] .

ArrayShorthand    = Name "[" Expression { "," Expression } "]" [ ":" Type ] .
```

## 2.7 字面量  (spec line 556)

```ebnf
Literal = IntegerLiteral
        | FloatLiteral
        | ArrayLiteral
        | CharacterLiteral
        | StringLiteral
        | FunctionLiteral
        | BooleanLiteral
        | IllegalLiteral
        | InfiniteLiteral
        | UninitializedLiteral .
```

## 2.7.1 整数  (spec line 587)

```ebnf
IntegerLiteral = DecimalLiteral | HexLiteral | BinaryLiteral .

DecimalLiteral = Digit { Digit | "_" } .
HexLiteral     = "0x" HexDigit { HexDigit | "_" } .
BinaryLiteral  = "0b" ("0" | "1") { ("0" | "1") | "_" } .

Digit    = "0" .. "9" .
HexDigit = Digit | "A" .. "F" | "a" .. "f" .
```

## 2.7.2 浮点数  (spec line 666)

```ebnf
FloatLiteral = DecimalLiteral "." DecimalLiteral [ Exponent ] .

Exponent = ("e" | "E") [ "+" | "-" ] DecimalLiteral .
```

## 2.7.3 布尔值  (spec line 724)

```ebnf
BooleanLiteral = "true" | "false" .
```

## 2.7.4 字符  (spec line 737)

```ebnf
CharacterLiteral = "'" ( Character | EscapeSequence ) "'" [ ":" CharacterEncodingTag ] .
```

## 2.7.5 字符串  (spec line 796)

```ebnf
StringLiteral = NormalStringLiteral | VerbatimStringLiteral .

NormalStringLiteral       = '"' { Character | EscapeSequence } '"' [ ":" CharacterEncodingTag ] .
VerbatimStringLiteral = '@"' { Character | '""' } '"' [ ":" CharacterEncodingTag ] .
```

## 2.7.6 编码与编码标注  (spec line 889)

```ebnf
CharacterEncodingTag = "ascii"
                     | "utf8"
                     | "utf16"
                     | "utf32"
                     | "ucs2"
                     | "ucs4" .
```

## 2.7.7 转义序列  (spec line 913)

```ebnf
EscapeSequence = "\" ( "a" | "b" | "f" | "n" | "r" | "t" | "v" | "\" | '"' | "'" | "0" )
               | "\x" HexDigit HexDigit
               | "\u" HexDigit HexDigit HexDigit HexDigit
               | "\U" HexDigit HexDigit HexDigit HexDigit HexDigit HexDigit HexDigit HexDigit .
```

## 2.7.8 数组  (spec line 948)

```ebnf
ArrayLiteral = "{" [ Element { "," Element } [ "," ] ] "}" .

Element      = Expression | RepeatElement | RangeElement .

RepeatElement = "[" Expression ":" Expression "]" .

RangeElement  = "[" Expression [ ".." Expression ] ( ".." | "..!" ) Expression "]" .
```

## 2.7.9 函数  (spec line 1040)

```ebnf
FunctionLiteral = "(" [ LiteralParameters ] [ ";" CallingConventionExpression ] ")" [ Type ] "=>" Block .
```

## 2.7.10 无穷值  (spec line 1192)

```ebnf
InfiniteLiteral = "infinite" .
```

## 2.7.11 非法值  (spec line 1232)

```ebnf
IllegalLiteral = "illegal" .
```

## 2.7.12 未初始化  (spec line 1271)

```ebnf
UninitializedLiteral = "uninitialized" .
```

## 3.1 类型构造语法  (spec line 1462)

```ebnf
Type = ConstructedType | BasicType | Identifier | MonomorphizationExpression .
```

## 3.2 基本类型  (spec line 1512)

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

## 3.6 类型构造器  (spec line 1717)

```ebnf
ConstructedType = TypeConstructor [ ConstructTargetType ] .

TypeConstructor = ArrayTypeConstructor
                | FunctionTypeConstructor
                | PointerTypeConstructor .

ConstructTargetType = BasicType | Type .
```

## 3.6.1 数组类型构造器  (spec line 1760)

```ebnf
ArrayTypeConstructor = "array(" Expression { "," Expression } ")" .
```

## 3.6.2 指针类型构造器  (spec line 1864)

```ebnf
PointerTypeConstructor = "pointer" .
```

## 3.6.3 函数类型构造器  (spec line 1917)

```ebnf
FunctionTypeConstructor = "function" [ "(" [ ParameterList ] [ ";" CallingConventionExpression ] ")" ] .

ParameterList = Parameters [ "," "wildcard" ] | "wildcard" .

CallingConventionExpression = Expression .
```

## 3.7.1 类  (spec line 2086)

```ebnf
ClassDeclaration = { Modifier } "class" Name [ GenericParameters ] [ BaseList ] Block .
```

## 3.7.2 结构体  (spec line 2317)

```ebnf
StructureDeclaration = { Modifier } "structure" Name [ GenericParameters ] Block .
```

## 3.7.3 接口  (spec line 2477)

```ebnf
InterfaceDeclaration = { Modifier } "interface" Name [ GenericParameters ] [ BaseList ] Block .
```

## 3.7.4 枚举  (spec line 2635)

```ebnf
EnumerationDeclaration = { Modifier } "enumeration" Name ":" BasicType "{" { EnumerationMemberDeclaration | ExtensionDeclaration } "}" .

EnumerationMemberDeclaration = Name "=" Expression ";" .
```

## 3.7.5 变体  (spec line 2766)

```ebnf
VariantDeclaration = { Modifier } "variant" Name [ GenericParameters ] "{" { VariantMemberDeclaration | ExtensionDeclaration } "}" .

VariantMemberDeclaration = Name [ ":" Type ] ";" .
```

## 3.7.6 共同体  (spec line 2919)

```ebnf
UnionDeclaration = { Modifier } "union" Name [ GenericParameters ] "{" { UnionMemberDeclaration | ExtensionDeclaration } "}" .

UnionMemberDeclaration = { Modifier } Name ":" Type ";" .
```

## 3.7.7 继承列表  (spec line 3047)

```ebnf
BaseList = ":" ( Identifier | MonomorphizationExpression ) { "," ( Identifier | MonomorphizationExpression ) } .
```

## 3.8.1 字段  (spec line 3101)

```ebnf
FieldDefinition = { Modifier } Name [ ":" Type ] [ "=" Expression | Block ] .
```

## 3.8.2 访问器  (spec line 3180)

```ebnf
AccessorDefinition        = { Modifier } "accessor" Name ":" Type [ "=>" Backing ] [ AccessorMembers ] .

AccessorMembers           = "{" [ AccessorMemberDefinition { ";" AccessorMemberDefinition } [ ";" ] ] "}" .

AccessorMemberDefinition  = { Modifier } ( "get" | "set" ) [ ":" Type ] [ "=" Expression ]
                          | { Modifier } "get" "(" [ "this" ":" "&" Type ] ")" Block
                          | { Modifier } "set" "(" [ "this" ":" "&" Type "," ] "value" [ ":" Type ] ")" Block .

Backing                   = Identifier .
```

## 3.8.3 扩展  (spec line 3418)

```ebnf
ExtensionDeclaration = "extension" [ "for" ExtensionTarget ] Block .

ExtensionTarget      = Type
                     | GenericTarget
                     | ArrayTarget .

GenericTarget        = Identifier "<" GenericParameterDefinition { ";" GenericParameterDefinition } ">" .

ArrayTarget          = "array" "(" TargetBinding { "," TargetBinding } ")" GenericParameterDefinition .

TargetBinding        = Name ":" Type .
```

## 4. 表达式  (spec line 3544)

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

## 4.1 值表达式  (spec line 3634)

```ebnf
ValueExpression = Identifier
                | Literal
                | MemberAccessExpression
                | PointerMemberAccessExpression
                | MonomorphizationExpression
                | IndexAccessExpression .
```

## 4.2 括号表达式  (spec line 3665)

```ebnf
ParenthesisExpression = "(" Expression ")" .
```

## 4.3 类型转换表达式  (spec line 3682)

```ebnf
TypeConversionExpression = "Cast" "<" Type ">" "(" Expression ")" .
```

## 4.4 访问表达式  (spec line 3756)

```ebnf
AccessExpression = MemberAccessExpression
                 | PointerMemberAccessExpression
                 | MonomorphizationExpression
                 | IndexAccessExpression .
```

## 4.4.1 成员访问表达式  (spec line 3777)

```ebnf
MemberAccessExpression = ( Identifier | Expression ) "." Name .
```

## 4.4.2 指针成员访问表达式  (spec line 3823)

```ebnf
PointerMemberAccessExpression = ( Identifier | Expression ) "->" Name .
```

## 4.4.3 索引访问表达式  (spec line 3857)

```ebnf
IndexAccessExpression = ( Identifier | Expression ) "[" Expression "]" .
```

## 4.5 函数调用表达式  (spec line 3902)

```ebnf
FunctionCallExpression = Expression "(" Arguments ")" .
```

## 4.6 构造表达式  (spec line 4016)

```ebnf
ConstructionExpression = ConstructedTypeName "(" Arguments ")" .

ConstructedTypeName = Type | MonomorphizationExpression .
```

## 4.7 成员构造表达式  (spec line 4114)

```ebnf
MemberConstructionExpression = ConstructedTypeName "." Name "(" Expression ")" .
```

## 4.8 is 表达式  (spec line 4229)

```ebnf
IsExpression = Expression "is" IsTarget .

IsTarget = Identifier | SpecialFloatTarget .

SpecialFloatTarget = IllegalLiteral | InfinityTarget .

InfinityTarget = [ "-" ] "infinite" .
```

## 4.9 运算符  (spec line 4339)

```ebnf
OperatorDefinition = { Modifier } "operator" OperatorHead [ "=" Expression | Block | AccessorMembers ] .

OperatorHead      = OperatorShorthand | OperatorNotation [ ":" Type ] .

OperatorShorthand = OperatorSymbol "(" [ Parameters ] ")" [ ":" Type ] .

OperatorNotation  = OperatorSymbol | IndexerNotation | ConstructedTypeName .

OperatorSymbol = "+" | "-" | "*" | "/" | "%" | "==" | ">" | "<" | "??" .

IndexerNotation = "[" Parameters "]" .
```

## 4.9.3 取负表达式  (spec line 4702)

```ebnf
NegateExpression = "-" Expression .
```

## 4.9.4 逻辑非表达式  (spec line 4748)

```ebnf
LogicalNotExpression = "!" Expression .
```

## 4.9.5 按位取反表达式  (spec line 4768)

```ebnf
BitwiseNotExpression = "^" Expression .
```

## 4.9.6 引用表达式  (spec line 4791)

```ebnf
ReferenceExpression = "&" Expression .
```

## 4.9.7 解引用表达式  (spec line 4846)

```ebnf
DereferenceExpression = "*" Expression .
```

## 4.9.8 乘法表达式  (spec line 4866)

```ebnf
MultiplyExpression = Expression "*" Expression .
```

## 4.9.9 除法表达式  (spec line 4888)

```ebnf
DivideExpression = Expression "/" Expression .
```

## 4.9.10 求余表达式  (spec line 4910)

```ebnf
RemainderExpression = Expression "%" Expression .
```

## 4.9.11 加法表达式  (spec line 4932)

```ebnf
AddExpression = Expression "+" Expression .
```

## 4.9.12 减法表达式  (spec line 4954)

```ebnf
MinusExpression = Expression "-" Expression .
```

## 4.9.13 逻辑左移表达式  (spec line 4976)

```ebnf
LogicalLeftShiftExpression = Expression "<<" Expression .
```

## 4.9.14 逻辑右移表达式  (spec line 5002)

```ebnf
LogicalRightShiftExpression = Expression ">>" Expression .
```

## 4.9.15 小于表达式  (spec line 5028)

```ebnf
LessExpression = Expression "<" Expression .
```

## 4.9.16 大于表达式  (spec line 5050)

```ebnf
GreaterExpression = Expression ">" Expression .
```

## 4.9.17 小于等于表达式  (spec line 5072)

```ebnf
LessEqualExpression = Expression "<=" Expression .
```

## 4.9.18 大于等于表达式  (spec line 5097)

```ebnf
GreaterEqualExpression = Expression ">=" Expression .
```

## 4.9.19 相等表达式  (spec line 5122)

```ebnf
EqualExpression = Expression "==" Expression .
```

## 4.9.20 不等表达式  (spec line 5144)

```ebnf
NotEqualExpression = Expression "!=" Expression .
```

## 4.9.21 按位与表达式  (spec line 5167)

```ebnf
BitwiseAndExpression = Expression "&" Expression .
```

## 4.9.22 按位异或表达式  (spec line 5191)

```ebnf
BitwiseXorExpression = Expression "^" Expression .
```

## 4.9.23 按位或表达式  (spec line 5215)

```ebnf
BitwiseOrExpression = Expression "|" Expression .
```

## 4.9.24 逻辑和表达式  (spec line 5239)

```ebnf
LogicalAndExpression = Expression "&&" Expression .
```

## 4.9.25 逻辑或表达式  (spec line 5259)

```ebnf
LogicalOrExpression = Expression "||" Expression .
```

## 4.9.26 空值合并表达式  (spec line 5279)

```ebnf
NullCoalescingExpression = Expression "??" CoalescingAlternative .

CoalescingAlternative = Expression
                      | "throw" [ Expression ]
                      | "return" [ Expression ] .
```

## 4.9.27 条件表达式  (spec line 5385)

```ebnf
ConditionalExpression = Expression "?" Expression ":" Expression .
```

## 5. 语句  (spec line 5448)

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

## 5.1 代码块  (spec line 5534)

```ebnf
Block = "{" { Statement | Block } "}" .
```

## 5.2 local  (spec line 5587)

```ebnf
LocalDefinition = { Modifier } "local" LocalItem { "," LocalItem } .

LocalItem       = Name [ ":" Type ] [ "=" Expression ] .
```

## 5.3 fixed  (spec line 5671)

```ebnf
FixedDefinition = { Modifier } "fixed" Name [ GenericParameters ] [ ":" Type ] [ "=" Expression | Block ] .
```

## 5.4 heap  (spec line 5758)

```ebnf
HeapDefinition = { Modifier } "heap" [ "(" Expression ")" ] Name [ ":" Type ] [ "=" Expression ] .
```

## 5.5 linkable  (spec line 5863)

```ebnf
LinkableDefinition = { Modifier } "linkable" [ "(" StringLiteral ")" ] FunctionShorthand
                   | { Modifier } "linkable" [ "(" StringLiteral ")" ] Name [ GenericParameters ] [ ":" Type ] [ "=" Expression | Block ] .
```

## 5.6 use 语句  (spec line 5942)

```ebnf
UseStatement = "use" Identifier { "," Identifier } .
```

## 5.7 赋值语句  (spec line 5993)

```ebnf
SetStatement          = AssignmentTarget "=" Expression .

CompoundSetStatement  = AssignmentTarget ( "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ) Expression .

AssignmentTarget      = Identifier
                      | IndexAccessExpression
                      | PointerMemberAccessExpression .
```

## 5.8 dispose 语句  (spec line 6111)

```ebnf
DisposeStatement = "dispose" ( Identifier | PointerMemberAccessExpression ) .
```

## 5.9.1 逻辑控制流名称  (spec line 6209)

```ebnf
LogicalControlFlowName = ":" Name .
```

## 5.9.2 if 语句  (spec line 6250)

```ebnf
IfStatement = "if" Expression Block { "else" "if" Expression Block } [ "else" Block ] .
```

## 5.9.3 while 语句  (spec line 6324)

```ebnf
WhileStatement = "while" Expression [ ":" Name ] Block .
```

## 5.9.4 do while 语句  (spec line 6387)

```ebnf
DoWhileStatement = "do" [ ":" Name ] Block "while" Expression .
```

## 5.9.5 for 语句  (spec line 6439)

```ebnf
ForStatement = "for" [ "&" ] Name ":" Type "in" TraverseTarget [ ":" Name ] Block .

TraverseTarget = Expression | Range .

Range = Expression [ ".." Expression ] ( ".." | "..!" ) Expression .
```

## 5.9.6 match 语句  (spec line 6580)

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

## 5.9.7 break 语句  (spec line 6771)

```ebnf
BreakStatement = "break" [ Name ] .
```

## 5.9.8 continue 语句  (spec line 6828)

```ebnf
ContinueStatement = "continue" [ Name ] .
```

## 5.10.2 return 语句  (spec line 6935)

```ebnf
ReturnStatement = "return" [ Expression ] .
```

## 5.10.3 throw 语句  (spec line 7017)

```ebnf
ThrowStatement = "throw" Expression .
```

## 5.10.4 trap 语句  (spec line 7085)

```ebnf
TrapStatement  = "trap" TrapBody { "catch" CatchCondition CatchBlock } [ "finally" FinallyBlock ] .

TrapBody       = Block .
CatchBlock     = Block .
FinallyBlock   = Block .
CatchCondition = [ Name ":" ] Type
               | "wildcard" .
```

## 7. 模块  (spec line 7333)

```ebnf
ModuleDeclaration = { Modifier } "module" Name Block .
```

## 8.1 函数定义语法糖  (spec line 7383)

```ebnf
FunctionDeclarationSugar = { Modifier } "fixed" Name [ GenericParameters ] ":" FunctionTypeConstructor Block .
```

## 8.2 参数  (spec line 7496)

```ebnf
Parameters        = Parameter { "," Parameter } .

Parameter         = [ "#" ] [ Name ":" ] [ "&" ] Type [ "=" Expression ] .

LiteralParameters = LiteralParameter { "," LiteralParameter } .

LiteralParameter  = [ "#" ] Name [ ":" [ "&" ] Type ] .
```

## 8.3 实参  (spec line 7600)

```ebnf
Arguments = [ Argument { "," Argument } ] .

Argument = [ Name ":" ] Expression .
```

## 9. 修饰符  (spec line 7876)

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

## 9.1 可访问性修饰符  (spec line 7964)

```ebnf
AccessibilityModifier = BaseAccessibilityModifier | ScopedModifier .

BaseAccessibilityModifier = PublicAccessibilityModifier
                          | PrivateAccessibilityModifier
                          | ProtectedAccessibilityModifier .
```

## 9.2 public 修饰符  (spec line 8054)

```ebnf
PublicAccessibilityModifier = "public" .
```

## 9.3 private 修饰符  (spec line 8101)

```ebnf
PrivateAccessibilityModifier = "private" .
```

## 9.4 protected 修饰符  (spec line 8161)

```ebnf
ProtectedAccessibilityModifier = "protected" .
```

## 9.5 scoped 修饰符  (spec line 8223)

```ebnf
ScopedModifier = "scoped" "(" ScopedTarget { "," ScopedTarget } ")" .

ScopedTarget = [ "sealed" ] Identifier
             | "operator" [ Identifier | OperatorSymbol ] .
```

## 9.6 readonly 修饰符  (spec line 8374)

```ebnf
ReadonlyModifier = "readonly" .
```

## 9.7 implicitable 修饰符  (spec line 8391)

```ebnf
ImplicitableModifier = "implicitable" .
```

## 9.8 verbatim 修饰符  (spec line 8467)

```ebnf
VerbatimModifier = "verbatim" .
```

## 9.9 manual 修饰符  (spec line 8514)

```ebnf
ManualModifier = "manual" .
```

## 9.10 abstract 修饰符  (spec line 8531)

```ebnf
AbstractModifier = "abstract" .
```

## 9.11 sealed 修饰符  (spec line 8549)

```ebnf
SealedModifier = "sealed" .
```

## 9.12 virtual 修饰符  (spec line 8565)

```ebnf
VirtualModifier = "virtual" .
```

## 9.13 override 修饰符  (spec line 8581)

```ebnf
OverrideModifier = "override" .
```

## 9.14 aligned(N) 修饰符  (spec line 8597)

```ebnf
AlignedModifier = "aligned" "(" IntegerLiteral ")" .
```

## 9.15 static 修饰符  (spec line 8655)

```ebnf
StaticModifier = "static" .
```

## 10.1 泛型参数  (spec line 8788)

```ebnf
GenericParameters = "<" GenericParameterDefinition { ";" GenericParameterDefinition } ">" .

GenericParameterDefinition = GenericParameter [ ":" Constraint { "," Constraint } ] .

GenericParameter = Identifier .
Constraint       = TypeKind | Identifier .

TypeKind = "class" | "structure" | "interface" | "enumeration" | "variant" | "union" | "basic" .
```

## 10.2 单态化表达式  (spec line 8920)

```ebnf
MonomorphizationExpression = ( Identifier | Expression ) "<" [ Type ] { ";" [ Type ] } ">" .
```

## 11.3 编译期表达式  (spec line 9053)

```ebnf
CompileTimeExpression = "#" "(" Expression ")" .
```

## 11.4 编译期块  (spec line 9095)

```ebnf
CompileTimeBlock = "#" Block .
```

## 11.6 条件编译  (spec line 9227)

```ebnf
CompileTimeIf     = "#if" Expression CompileTimeBranch [ "else" CompileTimeBranch ] .

CompileTimeBranch = Block .
```

