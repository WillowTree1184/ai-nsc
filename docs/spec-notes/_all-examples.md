# All NovaSharp examples extracted from Draft 1.1 (90 blocks)

## 2.4 标识符  (spec line 369)

```novasharp
// 声明：标识符 class
local class: integer(4) = 1;

// 在当前作用域下，可以访问到标识符 class
// 认为标识符 class 已经被定义，遮蔽关键字 class

// 定义：类 MyClass
// 合法：必须在 class 前加入字符 @，以将 class 解析为关键字
@class MyClass { }

// 在当前作用域下，不可以访问到标识符 structure
// 认为关键字 structure 没有被遮蔽

// 定义：结构体 MyStructure
// 合法：仍然可以在 structure 前加入字符 @，将 structure 解析为关键字
@structure MyStructure { }

// 定义：类 integer
class integer { }

// 在当前作用域下，integer 遮蔽基本类型关键字

// 合法：必须加入 @ 才能解析为基本类型
local x: @integer(4);

// 非法：@ 与标识符之间存在空白
// @ class A { }
// 错误：字符 @ 与标识符之间不得存在空白
```

## 2.5 标识符定义简写  (spec line 467)

```novasharp
// 合法：函数简写
fixed Main(): integer(4)
{
    return 0;
}

// 合法：零参数、空返回类型
fixed Foo()
{
}

// 非法：返回类型为空，return 0 类型不匹配
// fixed Bad()
// {
//     return 0;
// }

// 合法：数组简写
local a[5]: integer(4);
local b[5, 5]: integer(4);

// 非法：数组元素类型为空
// local c[5];

// 合法：需要推断时使用非简写定义
local d = 0;

// 非法：函数简写与数组简写组合
// local e[5](): integer(4);

// 合法：显式形式与简写等价
fixed Explicit: function() integer(4) { return 0; }
fixed Shorthand(): integer(4) { return 0; }

// 合法：循环参数简写
local matrix: array(5) array(5) integer(4);

for row[5]: integer(4) in matrix
{
    // row: array(5) integer(4)
}
```

## 2.7.1 整数  (spec line 623)

```novasharp
// 合法：十进制整数字面量
local decimal: integer(4) = 42;

// 合法：带分隔符的十进制整数字面量
local decimalWithSeparator: integer(4) = 1_000_000;
// 等价于 1000000，分隔符编译期擦除

// 合法：十六进制整数字面量
local hex: integer(4) = 0xFF;

// 合法：二进制整数字面量
local binary: integer(4) = 0b1010;

// 合法：类型推断为 cardinal(4)
local cardinalValue: cardinal(4) = 0xFFFFFFFF;

// 合法：类型推断为 float(4)
local floatValue: float(4) = 100;
// 整数字面量在此上下文中推断为 float(4)

// 合法：默认类型推断
// 无显式类型约束时，编译器选择足以表示该字面量的最小合法字节数
local define = 127;
// 推断为 integer(1)

// 非法：十六进制包含非法字符
// local bad: integer(4) = 0xFG;
// 错误：意外的字符

// 非法：二进制包含非 0/1 字符
// local bad: integer(4) = 0b2010;
// 错误：意外的字符

// 非法：值超出目标类型范围
// local bad: integer(1) = 256;
// 错误：256 超出类型 integer(1) 的取值范围
```

## 2.7.2 浮点数  (spec line 697)

```novasharp
// 合法：基本浮点数字面量
local pi: float(4) = 3.14;

// 合法：带指数
local large: float(8) = 1.5e10;

// 合法：负指数
local small: float(4) = 2.5e-3;

// 非法：缺少小数部分
// local bad: float(4) = 1.;
// 错误：意外的字符：;

// 非法：指数后无数字
// local bad: float(4) = 1.0e;
// 错误：意外的字符：;

// 非法：值超出目标类型范围
// local bad: float(4) = 1.0e100;
// 错误：1.0e100 无法由 float(4) 表示
```

## 2.7.4 字符  (spec line 765)

```novasharp
// 合法：基本字符（默认 UTF-8）
local character: cardinal(4) = ' ';

// 合法：显式编码标注
local asciiCharacter: cardinal(1) = 'a':ascii;

// 合法：转义序列
local newline: cardinal(4) = '\n';

// 合法：默认推断
local c = '中';
// 推断为 cardinal(4)，UTF-8 编码

// 非法：空字符字面量
// local bad = '';
// 错误：缺少字符内容

// 非法：多字符
// local bad = 'ab';
// 错误：CharacterLiteral 只能包含一个字符或转义序列

// 非法：编码不支持的内容
// local bad: cardinal(1) = '中':ascii;
// 错误：内容超出 ascii 编码范围
```

## 2.7.5 字符串  (spec line 847)

```novasharp
// 合法：基本字符串（默认 UTF-8）
local hello[13]: cardinal(1) = "Hello, World!";

// 合法：普通字符串换行丢弃
local singleLine[27]: cardinal(1) = "第一行，
    还是第一行";
// 等价于 "第一行，还是第一行"
// 注意：行终止符前的空白字符会被保留

// 合法：转义序列
local withNewline[19]: cardinal(1) = "第一行\n第二行";

// 合法：显式编码标注
local explicitAscii[5]: cardinal(1) = "Hello":ascii;

// 合法：逐字字符串换行保留
local multiLine[33]: cardinal(1) = @"第一行,
    不是第一行了";
// 换行和缩进空白均保留

// 合法：逐字字符串转义失效
local path[13]: cardinal(1) = @"C:\Users\test";
// 内容等价于 C:\Users\test，反斜杠不视为转义

// 合法：逐字字符串连续引号
local quote[15]: cardinal(1) = @"He said ""Hello""";
// 内容等价于 He said "Hello"

// 非法：逐字字符串中 @ 与 " 之间存在空白
// local bad: array(5) cardinal(1) = @ "Hello";
// 错误：意外的字符：@

// 非法：编码不支持的内容
// local bad: array(1) cardinal(1) = "中":ascii;
// 错误：内容超出 ascii 编码范围
```

## 2.7.8 数组  (spec line 997)

```novasharp
// 合法：基本数组
local arr[3]: integer(1) = { 1, 2, 3 };

// 合法：嵌套数组
local nested[2, 2]: integer(1) = { { 1, 2 }, { 3, 4 } };

// 合法：尾随逗号
local trailing[2]: integer(1) = { 1, 2, };

// 合法：重复展开
local filled[4]: integer(4) = { 1, [2: 3] }; // 1, 2, 2, 2

// 合法：区间展开
local expanded[6]: integer(4) = { 1, [1..5] }; // 1, 1, 2, 3, 4, 5

// 合法：带步长的区间展开
local stepped[6]: integer(4) = { [0..2..10] }; // 0, 2, 4, 6, 8, 10

// 合法：空数组（有上下文约束）
fixed TakeArray(empty[0]: integer(4)) { }
TakeArray({});

// 非法：空数组无上下文约束
// local bad = {};
// 错误：无法推断元素类型

// 非法：元素类型不兼容
// local bad: array(2) integer(1) = { 1, 1.5 };
// 错误：元素类型不可隐式转换

// 定义：变量 count
local count: cardinal(4) = 3;

// 非法：展开参数不是编译期常量
// local bad: array(4) integer(4) = { [0: count] };
// 错误：展开参数必须为编译期常量
```

## 2.7.9 函数  (spec line 1089)

```novasharp
// 定义：无参函数
fixed GetAnswer(): integer(4) =
    () integer(4) =>
    {
        return 42;
    };

// 定义：多参函数
fixed Add(a: integer(4), b: integer(4)): integer(4) =
    (a: integer(4), b: integer(4)) integer(4) =>
    {
        return a + b;
    };

// 定义：显式调用约定
fixed CCallback(; CDeclaration): integer(4) =
    (; CDeclaration) integer(4) =>
    {
        return 0;
    };

// 示例：按值捕获
// 定义：函数 CaptureByValue（通过函数定义语法糖）
fixed CaptureByValue()
{
    local threshold: integer(4) = 10;

    fixed IsAbove(value: integer(4)): boolean =
        (value: integer(4)) boolean =>
        {
            // threshold 是值类型，捕获时复制数据
            return value > threshold; // 参与比较的是一份 threshold 的副本
        };

    threshold = 99; // 不会修改 IsAbove 中的副本
    local result: boolean = IsAbove(5); // result = false
}

// 定义：类 Counter（引用类型）
class Counter
{
    public Count: integer(4);
}

// 示例：按引用捕获
// 定义：函数 CaptureByReference（通过函数定义语法糖）
fixed CaptureByReference()
{
    local counter: Counter = Counter();
    counter.Count = 5;

    fixed GetCount(): integer(4) =
        () integer(4) =>
        {
            // counter 是引用类型，捕获时复制引用
            return counter.Count; // 访问的是对 counter 的引用副本，该副本仍然指向 counter
        };

    counter.Count = 20; // 修改同一个实例的 Count 成员
    local result: integer(4) = GetCount(); // result = 20
}

// 示例：this 参数
class Calculator
{
    private OffsetPointer: pointer integer(4);

    extension
    {
        // 定义：函数 CreateGetter
        public fixed CreateGetter(this: Calculator): function() integer(4) =
            (this: Calculator) function() integer(4) =>
            {
                return () integer(4) => { return *(this.OffsetPointer); };
            };
    }

    // 非法：参数列表中出现两个 this
    // extension
    // {
    //     fixed Bad: function(this: Calculator, this: Calculator) =
    //         (this: Calculator, this: Calculator) =>
    //         { }
    // }
    // 错误：this 在参数列表中最多出现一次
}

// 定义：函数 CreateGetterByInstance
public fixed CreateGetterByInstance(instance: Calculator): function() integer(4) =
        (this: Calculator) function() integer(4) =>
        {
            // 调用者只负责传值
            // 调用者和被调用者的参数名称不同，不影响函数类型和调用行为
            // 被调用者依然可以正常以 this 参数的方式访问 Calculator 实例
            return () integer(4) => { return *(this.OffsetPointer); };
        };
```

## 2.7.10 无穷值  (spec line 1213)

```novasharp
// 合法：以无穷值字面量产生正无穷
local upper: float(4) = infinite;

// 合法：以一元取负产生负无穷
local lower: float(4) = -infinite;

// 合法：无穷参与比较
local bigger: boolean = upper > lower; // true

// 非法：省略类型上下文导致推断失败
// local unknown = infinite;
// 错误：无穷值字面量的类型必须由编译器根据语句上下文推断
```

## 2.7.11 非法值  (spec line 1251)

```novasharp
// 合法：以非法值字面量产生非数
local ratio: float(4) = illegal;

// 合法：经 is 表达式判定非数
local bad: boolean = ratio is illegal;

// 非法：将非法值字面量用作比较操作数
// local never: boolean = ratio == illegal;
// 错误：禁止将非法值字面量用作比较表达式的操作数

// 非法：省略类型上下文导致推断失败
// local unknown = illegal;
// 错误：非法值字面量的类型必须由编译器根据语句上下文推断
```

## 2.7.12 未初始化  (spec line 1299)

```novasharp
// 合法：显式请求未初始化存储，并读取未初始化值
local seed: integer(4) = uninitialized;
local entropy: integer(4) = seed;

// 合法：赋值后调用
local callback() = uninitialized;
callback = () => { };

// 非法：作为常量初始值
// fixed bad: integer(4) = uninitialized;
// 错误：uninitialized 不得作为常量的初始值
```

## 2.8 语句终止  (spec line 1338)

```novasharp
// 定义：类 Counter
class Counter
{
    // 合法：分号终止
    private Count: integer(4);

    extension
    {
        // 合法：后大括号终止，其后仅包含空白与注释
        public fixed Value(): integer(4) { return Count; }
    }
}

// 非法：后大括号之后存在其它内容
// class Bad { } class Another { }
// 错误：后大括号之后仅可包含空白字符和（或）注释
```

## 3.1 类型构造语法  (spec line 1487)

```novasharp
// 合法：基本类型
local x: integer(4);

// 合法：构造类型
local arr: array(5) integer(4);

// 合法：嵌套构造类型
local table: array(3) array(4) boolean;

// 合法：函数类型
local f: function(value: integer(4));

// 合法：带泛型实参的类型
local box: Box<integer(4)>;

// 非法：泛型参数出现在类型中
// local bad: <T> function(value: T) T;
// 错误：泛型参数仅允许出现在声明中
```

## 3.2 基本类型  (spec line 1591)

```novasharp
// 合法：整数类型
local integer32: integer(4) = 42;

// 合法：基数类型
local cardinalByte: cardinal(1) = 255;

// 合法：浮点类型
local float64: float(8) = 3.14159;

// 合法：布尔类型
local flag: boolean = true;

// 合法：空类型作为函数返回类型
local DoNothing() = () => { };

// 合法：显式转换为浮点
local i: integer(4) = 100;
local f: float(8) = (i as float(8));

// 合法：显式转换为布尔
local zero: integer(4) = 0;
local b: boolean = (zero as boolean); // false

// 非法：省略括号
// local bad: integer = 42;
// 错误：不被识别为基本类型

// 非法：不支持的 N 值
// local bad: integer(3) = 42;
// 错误：N 不合法

// 非法：float 不支持的 N 值
// local bad: float(16) = 1.0;
// 错误：N 不合法

// 非法：整数隐式转换为浮点
// local bad: float(4) = i;
// 错误：基本类型之间不得存在隐式转换

// 非法：整数隐式转换为布尔
// local bad: boolean = zero;
// 错误：基本类型之间不得存在隐式转换
```

## 3.5 引用  (spec line 1692)

```novasharp
// 定义：类 Counter
class Counter
{
    public Count: integer(4);
}

// 合法：标识符存储引用，引用指向实例本体
local counter: Counter = Counter();

// 合法：多个引用指向同一实例
local another: Counter = counter;

// 合法：对引用的操作映射为对实例本体的操作
another.Count = 5; // counter.Count 亦为 5

// 非法：显式解引用
// (*counter).Count = 5;
// 错误：引用不得被显式解引用
```

## 3.6.1 数组类型构造器  (spec line 1835)

```novasharp
// 合法：基本数组
local arr: array(5) integer(4);

// 合法：多维数组
local matrix: array(3, 4) boolean;
// 等价于：array(3) array(4) boolean

// 合法：数组定义简写与数组参数展开语法糖叠加
// 数组定义简写展开为 message: array(length: cardinal(4)) cardinal(1)
fixed LogNote(message[length: cardinal(4)]: cardinal(1))
{
}

// 合法：数组显式转指针
local p: pointer integer(4) = (arr as pointer integer(4));

// 合法：指针显式转定长数组
local back: array(5) integer(4) = (p as array(5) integer(4));

// 非法：数组大小为负
// local bad: array(-1) integer(4);
// 错误：数组大小必须为非负整数
```

## 3.6.2 指针类型构造器  (spec line 1897)

```novasharp
// 合法：基本指针
local p: pointer integer(4);

// 合法：省略目标类型的指针
local raw: pointer;

// 合法：指针类型转换
local pf: pointer float(4) = (p as pointer float(4));

// 非法：省略目标类型的指针解引用
// local pv: pointer;
// local x: integer(4) = *pv;
// 错误：不得对省略目标类型的指针进行解引用
```

## 3.6.3 函数类型构造器  (spec line 1999)

```novasharp
// 合法：无名参数的类型
local Callback: function(integer(4));

// 合法：有名参数的类型
local Named: function(value: integer(4));

// 合法：返回整数
local Safe: function() integer(4);

// 合法：携带默认值的参数必须携带参数名
local WithDefault: function(value: integer(4) = 0) integer(4);

// 合法：无名引用参数
local WithRef: function(&integer(4));

// 合法：有名引用参数
local NamedRef: function(value: &integer(4));

// 合法：无名编译期常量参数
local CompileTime: function(#integer(4));

// 合法：有名编译期常量参数
local NamedCompileTime: function(#value: integer(4));

// 非法：编译期常量参数与普通参数的类型不同
// local Bad: function(integer(4)) = CompileTime;

// 定义：类 MyClass
// 合法：this 参数
local WithThis: function(this: MyClass);

// 合法：wildcard 单独作为最后一项
local AnyArgs: function(wildcard);

// 合法：wildcard 跟在固定参数之后
local PrefixedAny: function(value: integer(4), wildcard);

// 合法：固定参数前缀与调用约定
local PrefixedAnyC: function(value: integer(4), wildcard; CDeclaration);

// 合法：显式调用约定
local Cdecl: function(value: integer(4); CDeclaration);

// 合法：空参数列表与调用约定
local CdeclEmpty: function(; CDeclaration);

// 合法：将 function(wildcard) 类型转换为 function(boolean) 类型
local Foo: function(value: boolean) = (AnyArgs as function(boolean));
// 危险：该转换不校验目标签名

// 合法：推断出的函数类型保留参数名
fixed anotherFunction(a: integer(4)): integer(4)
{
    return a;
}

local foo = (anotherFunction as function(a: integer(4)));
// foo 的类型为 function(a: integer(4))

// 非法：省略参数类型标注
// local Bad: function(value);
// 错误：函数类型构造器的参数类型必须出现

// 非法：无名参数携带默认值
// local Bad: function(integer(4) = 0);
// 错误：默认值表达式不得在参数名省略时出现

// 非法：携带默认值表达式的参数后出现无默认值表达式的参数
// local Bad: function(value: integer(4) = 0, flag: boolean) integer(4);
// 错误：在有默认值参数后发现意外的无默认值参数

// 非法：wildcard 不是最后一项
// local Bad: function(wildcard, value: integer(4));
// 错误：wildcard 必须仅作为参数列表的最后一项

// 非法：wildcard 位于参数中间
// local Bad: function(value: integer(4), wildcard, flag: boolean);
// 错误：wildcard 必须仅作为参数列表的最后一项
```

## 3.7.1 类  (spec line 2233)

```novasharp
// 合法：抽象类
abstract class Animal
{
    extension
    {
        public virtual fixed Speak(this: Animal)
        {

        }
    }
}

// 合法：继承与重写
class Cat: Animal
{
    extension
    {
        public override fixed Speak(this: Cat)
        {
            // 合法：绕过分派，调用基类方法
            base.Speak();
        }
    }
}

// 合法：密封类
sealed class Ragdoll: Cat
{
    extension
    {
        public override fixed Speak(this: Ragdoll)
        {

        }
    }
}

// 合法：隐式继承 Object
class Simple
{

}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：动态分派
    local animal: Animal = Cat();
    animal.Speak(); // 执行 Cat.Speak

    return 0;
}

// 非法：继承密封类
// class Bad: Ragdoll { }
// 错误：不得继承 sealed 类

// 非法：未提供泛型实参
// class Bad: Box { }
// 错误：泛型类被继承时必须提供泛型实参

// 非法：重写方法类型不匹配
// class Bad: Animal
// {
//     extension
//     {
//         public override fixed Speak: function(this: Bad) integer(4) { return 0; }
//     }
// }
// 错误：重写方法类型必须与被重写方法相同

// 非法：类声明体内定义变量
// class Bad
// {
//     public local X: integer(4);
// }
// 错误：类声明体内不得定义变量，必须使用字段定义
```

## 3.7.2 结构体  (spec line 2412)

```novasharp
// 合法：基本结构体
structure Vector2D
{
    public X: float(4) = 0;
    public Y: float(4) = 0;
}

// 合法：紧凑布局结构体
// 全部字段默认按 0 字节对齐
aligned(0) structure Compact
{
    public A: Vector2D;
    public B: Vector2D;
}

// 合法：对齐修饰
structure CEFHeader
{
    public Identifier[4]: integer(1);
    public Type: integer(1);
    public Arch: integer(2);
    public Offset: pointer integer(1);
    public aligned(4) Length: integer(4);
}

// 合法：泛型结构体
structure Box<T>
{
    public Value: T;
}

// 合法：值类型方法，this 参数必须为引用
structure Counter
{
    private _count: integer(4);

    extension
    {
        public fixed Increment(this: &Counter)
        {
            this._count = this._count + 1;
        }
    }
}

// 非法：结构体继承
// structure Bad: Vector2D { }
// 错误：结构体不得携带继承列表

// 非法：结构体包含虚方法
// structure Bad
// {
//     extension
//     {
//         public virtual fixed Foo: function(this: &Bad) { }
//     }
// }
// 错误：结构体成员不得以 virtual 修饰
```

## 3.7.3 接口  (spec line 2556)

```novasharp
// 合法：泛型接口
interface IValue<T>
{
    public fixed Get(this: IValue<T>): T;
    public accessor Value: T { get; }
}

// 合法：继承接口
interface IArray<T>: IValue<pointer T>
{
    public fixed Length(this: IArray<T>): integer(8);
}

// 定义：类 Box
// 合法：实现接口
class Box: IValue<integer(4)>
{
    public Data: integer(4);

    extension
    {
        public fixed Get(this: Box): integer(4)
        {
            return Data;
        }

        public accessor Value: integer(4) => Data { get; }
    }
}

// 合法：接口类型承载
local value: IValue<integer(4)> = Box();

// 合法：空接口不要求扩展块
interface Empty { }
class EmptyBox: Empty { }

// 非法：接口实例化
// local bad: IValue<integer(4)> = IValue<integer(4)>();
// 错误：接口不得被创建实例

// 非法：结构体实现接口
// structure Bad: IValue<integer(4)> { }
// 错误：接口必须仅由 class 实现

// 非法：接口成员为字段
// interface Bad
// {
//     public Value: integer(4);
// }
// 错误：接口成员不得为字段

// 非法：接口成员以 static 修饰
// interface Bad
// {
//     public static fixed Value: function() integer(4);
// }
// 错误：接口成员不得以 static 修饰

// 非法：接口成员被初始化
// interface Bad
// {
//     public fixed Value: integer(4) = 0;
// }
// 错误：接口内的常量不得被初始化

// 非法：实现成员位于扩展块之外
// class Bad: IValue<integer(4)>
// {
//     public fixed Get: function(this: Bad) integer(4) { return 0; }
// }
// 错误：接口成员必须在类型体扩展块中实现
```

## 3.7.4 枚举  (spec line 2703)

```novasharp
// 合法：基本枚举
enumeration Status: integer(4)
{
    Pending = 0;
    Running = 1;
    Completed = 2;

    extension
    {
        public fixed IsDone(this: &Status): boolean
        {
            return this == Status.Completed;
        }
    }
}

// 定义：变量 current
// 合法：使用枚举类型名省略语法糖
// 此时 Status.Running 被省略为 Running
local current: Status = Running;

// 合法：隐式转换
local s: Status = Status.Running;
local i: integer(4) = s;

// 合法：显式转换
local another: Status = (2 as Status);

// 非法：成员值重复
// enumeration Bad: integer(4)
// {
//     A = 1;
//     B = 1;
// }
// 错误：成员值不得重复

// 非法：省略底层类型
// enumeration Bad
// {
//     A = 1;
// }
// 错误：底层类型不得省略

// 非法：省略成员值
// enumeration Bad: integer(4)
// {
//     A;
// }
// 错误：成员值表达式不得省略

// 非法：枚举携带泛型参数
// enumeration Bad<T>: integer(4)
// {
//     A = 1;
// }
// 错误：枚举不得携带泛型参数
```

## 3.7.5 变体  (spec line 2851)

```novasharp
// 合法：基本变体
variant Status
{
    Idle;
    Loading: integer(4);
    Done;

    extension
    {
        public fixed IsDone(this: &Status): boolean
        {
            return this == Status.Done;
        }
    }
}

// 合法：泛型变体
variant Outcome<TError>
{
    Ok;
    Error: TError;
}

// 合法：无载荷状态
variant Phase
{
    Idle;
    Active: integer(4);
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：实例化并赋值
    local s: Status = Status.Loading(50);

    // 合法：经成员访问表达式取值并赋值
    s = Status.Done;

    // 合法：实例化新值并赋值
    s = Status.Idle;

    // 合法：比较
    local r1: Outcome<integer(4)> = Outcome<integer(4)>.Error(1);
    local r2: Outcome<integer(4)> = Outcome<integer(4)>.Error(1);
    local same: boolean = r1 == r2;

    return 0;
}

// 非法：未确定状态实例化
// local bad: Status = Status;
// 错误：变体实例化时必须确定状态

// 非法：状态不存在
// local bad: Status = Status.Unknown;
// 错误：Unknown 不是 Status 的状态

// 非法：载荷类型不匹配
// local bad: Status = Status.Loading("text");
// 错误：Loading 的载荷类型必须为 integer(4)
```

## 3.7.6 共同体  (spec line 2999)

```novasharp
// 定义：结构体 Color
structure Color
{
    public Value: cardinal(4);
}

// 合法：基本共同体
union Pixel
{
    RGBA: Color;
    Packed: cardinal(4);
    Channels[4]: integer(1);
}

// 合法：成员偏移
union AlignedBuffer
{
    Header: integer(1);
    aligned(8) Payload[64]: integer(1);
}

// 合法：共同体嵌入结构体
structure Frame
{
    public PixelData: Pixel;
    public Timestamp: integer(4);
}

// 非法：共同体成员携带初值
// union Bad
// {
//     Value: integer(4) = 0;
// }
// 错误：共同体成员不得携带初值

// 非法：共同体成员携带非 aligned 修饰符
// union Bad
// {
//     public Value: integer(4);
// }
// 错误：共同体成员必须仅支持 aligned(N) 修饰符
```

## 3.7.7 继承列表  (spec line 3069)

```novasharp
// 定义：类 MyBaseClass
class MyBaseClass
{

}

// 定义：接口 MyInterface
interface MyInterface
{

}

// 合法：类继承一个类并实现一个接口
class MyClass: MyBaseClass, MyInterface
{

}

// 合法：接口继承接口
interface MyOtherInterface: MyInterface
{

}
```

## 3.8.1 字段  (spec line 3143)

```novasharp
// 定义：类 MyClass
class MyClass
{
    // 合法：未初始化的字段
    public Delegate: function();

    // 合法：带初始值的字段
    public Count: integer(4) = 0;

    // 合法：函数字段
    public Inc: function(value: integer(4)) integer(4) { }
}

// 非法：字段定义位于类与结构体之外
// public Bad: integer(4);
// 错误：字段定义必须仅作为类或结构体的成员出现

// 非法：接口中的字段
// interface Bad
// {
//     public Value: integer(4);
// }
// 错误：接口成员不得为字段

// 非法：共同体中使用字段
// union Bad
// {
//     public Value: integer(4)
// }
// 错误：共同体成员不得为字段
```

## 3.8.2 访问器  (spec line 3287)

```novasharp
// 定义：类 Counter
class Counter
{
    private _count: integer(4);

    extension
    {
        // 合法：字段型访问器，默认读写 _count
        public accessor Count: integer(4) => _count;

        // 合法：字段型访问器，set 私有
        public accessor ReadOnlyCount: integer(4) => _count { get; private set; }
    }
}

// 定义：类 AutoCounter
class AutoCounter
{
    private _count: integer(4) = 0;

    extension
    {
        // 合法：字段型访问器，默认读写
        public accessor Count: integer(4) => _count;
    }
}

// 定义：类 FixedCounter
class FixedCounter
{
    extension
    {
        private fixed _count: integer(4) = 0;

        // 合法：常量 backing，自动只读
        public accessor Count: integer(4) => _count;
    }
}

// 定义：结构体 Vector3
structure Vector3
{
    public X: float(4);
    public Y: float(4);
    public Z: float(4);

    extension
    {
        // 合法：计算型访问器
        public accessor LengthSquared: float(4)
        {
            get = (this: &Vector3) float(4) => { return this.X * this.X + this.Y * this.Y + this.Z * this.Z; }
        }
    }
}

// 定义：函数 GetPlatform
fixed GetPlatform(): integer(4)
{
    return 0;
}

// 合法：无 this 的 get 块简写
accessor Platform: integer(4)
{
    get()
    {
        return GetPlatform();
    }
}

// 合法：扩展块中的 get/set 块简写
class PlatformHolder
{
    private _platform: integer(4);

    extension
    {
        public accessor Platform: integer(4) => _platform
        {
            get(this: &PlatformHolder)
            {
                return this._platform;
            }

            set(this: &PlatformHolder, value)
            {
                this._platform = value;
            }
        }
    }
}

// 合法：类型外的字段型访问器
local Global: integer(4);
accessor Hidden: integer(4) => Global;

// 定义：类 StaticCounter
class StaticCounter
{
    private static _count: integer(4);

    extension
    {
        // 合法：类型级访问器
        public static accessor Count: integer(4) => _count;
    }
}

// 非法：既无 Backing 又无成员
// accessor Bad: integer(4);
// 错误：访问器必须至少提供 Backing 或访问器成员

// 非法：计算型访问器成员没有实现
// accessor Bad: integer(4) { get; }
// 错误：无 Backing 的访问器成员必须提供实现表达式

// 非法：常量 backing 携带 set
// accessor Bad: integer(4) => _constValue { get; set; }
// 错误：常量 backing 的访问器不得包含 set

// 非法：访问器携带泛型参数
// accessor Bad<T>: T => _value;
// 错误：访问器不得携带泛型参数
```

## 3.8.3 扩展  (spec line 3479)

```novasharp
// 定义：结构体 Vector3
structure Vector3
{
    public X: float(4);
    public Y: float(4);
    public Z: float(4);

    // 合法：类型体内的扩展
    extension
    {
        public accessor Sum: float(4)
        {
            get = (this: &Vector3) float(4) => { return this.X + this.Y + this.Z; }
        }
    }
}

// 合法：外部扩展追加普通成员
extension for Vector3
{
    public fixed DoubledSum(this: &Vector3): float(4)
    {
        return this.Sum * 2;
    }
}

// 合法：绑定数组的大小与元素类型
extension for array(length: cardinal(4)) T
{
    public accessor Length: cardinal(4)
    {
        get = (this: &array(length) T) cardinal(4) => { return length; }
    }
}

// 合法：泛型目标
extension for MyClass<T>
{
}

// 合法：泛型目标并加以约束
extension for MyType<T: IIterable>
{
}

// 非法：在扩展中声明字段
// extension for Vector3
// {
//     public W: float(4);
// }
// 错误：扩展成员不得声明字段

// 非法：与已有成员重名
// extension for Vector3
// {
//     public fixed X: function(this: &Vector3) float(4) { return 0; }
// }
// 错误：扩展成员与已有成员重名
```

## 4.3 类型转换表达式  (spec line 3711)

```novasharp
// 定义：函数 Main
fixed Main(): integer(4)
{
    local a: integer(4) = 5;

    // 合法：基本类型之间的显式转换
    local b: integer(2) = (a as integer(2));

    // 合法：布尔到数值的显式转换
    local flag: boolean = true;
    local n: integer(4) = (flag as integer(4)); // 1

    return 0;
}

// 定义：结构体 HSLColor
structure HSLColor
{
    // ...
}

// 定义：结构体 RGBColor
structure RGBColor
{
    extension
    {
        // 定义：转换运算符
        // 合法：转换目标类型与返回值类型一致
        operator RGBColor: function(from: HSLColor) RGBColor
        {
            // ...
        }
    }
}

// 合法：优先选取用户定义的转换运算符
local hsl: HSLColor = HSLColor();
local rgb: RGBColor = (hsl as RGBColor);
```

## 4.5 函数调用表达式  (spec line 3931)

```novasharp
// 定义：函数 Calculate
fixed Calculate(a: integer(4), b: integer(4), c: integer(4) = 0): integer(4)
{
    return a + b + c;
}

// 定义：函数 Scale
fixed Scale(value: &integer(4), factor: integer(4))
{
    value = value * factor;
}

// 定义：函数 Invoke
fixed Invoke(callback: function())
{
    callback();
}

fixed Main(): integer(4)
{
    // 合法：基本位置参数调用
    local result: integer(4) = Calculate(1, 2, 3); // result = 6

    // 合法：省略默认参数
    local partial: integer(4) = Calculate(1, 2); // c 使用默认值 0，result = 3

    // 合法：命名参数调用
    local named: integer(4) = Calculate(a: 1, b: 2, c: 3);

    // 合法：命名参数乱序
    local reordered: integer(4) = Calculate(c: 3, a: 1, b: 2);

    // 合法：混合调用，位置参数在前，命名参数在后
    local mixed: integer(4) = Calculate(1, b: 2, c: 3);

    // 合法：引用参数调用
    local value: integer(4) = 5;
    Scale(value, 3); // value 变为 15

    // 合法：函数字面量直接调用
    local direct: integer(4) =
        (x: integer(4), y: integer(4)) integer(4) =>
        {
            return x * y;
        } (3, 4); // result = 12

    // 合法：无参调用
    Invoke(() => { });

    // 非法：命名参数后接位置参数
    // local bad: integer(4) = Calculate(a: 1, 2, 3);
    // 错误：命名参数后的所有参数必须携带参数名

    // 定义：函数 PrintNothing
    fixed PrintNothing()
    {
        return;
    }

    // 合法：经函数指针调用
    local functionPrinter: pointer function() = &PrintNothing;
    (*functionPrinter)();

    // 非法：重复指定同一参数
    // local bad: integer(4) = Calculate(1, a: 2, b: 3);
    // 错误：参数 a 被多次指定

    // 非法：省略无默认值的参数
    // local bad: integer(4) = Calculate(1);
    // 错误：参数 b 无默认值且未被提供

    // 非法：调用非函数表达式
    // local value: integer(4) = 1;
    // local bad: integer(4) = value(1, 2);
    // 错误：被调用者表达式的返回值类型必须为函数类型

    return 0;
}
```

## 4.6 构造表达式  (spec line 4059)

```novasharp
// 定义：结构体 Point
structure Point
{
    public X: integer(4);
    public Y: integer(4);

    extension
    {
        public fixed Construct(this: &Point, x: integer(4), y: integer(4))
        {
            this.X = x;
            this.Y = y;
        }
    }
}

// 定义：类 Cat
class Cat
{
    public Legs: integer(4);
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：构造值类型实例
    local origin: Point = Point(0, 0);

    // 合法：以默认构造函数构造引用类型实例
    local cat: Cat = Cat();

    // 合法：构造结果覆盖目标存储
    origin = Point(1, 2);

    return 0;
}

// 非法：接口不可构造
// local bad: IValue = IValue();
// 错误：被构造类型不得为接口

// 非法：实参不匹配
// local bad: Point = Point(1);
// 错误：没有与实参匹配的构造函数

// 非法：构造表达式指定内存分配方式
// local bad: Point = heap Point(1, 2);
// 错误：构造表达式不得指定内存分配方式
```

## 4.7 成员构造表达式  (spec line 4156)

```novasharp
// 定义：结构体 Color
structure Color
{
    public R: cardinal(1);
    public G: cardinal(1);
    public B: cardinal(1);
    public A: cardinal(1);

    extension
    {
        public fixed Construct(this: &Color, r: cardinal(1), g: cardinal(1), b: cardinal(1), a: cardinal(1))
        {
            this.R = r;
            this.G = g;
            this.B = b;
            this.A = a;
        }
    }
}

// 定义：共同体 Pixel
union Pixel
{
    RGBA: Color;
    Packed: cardinal(4);
    Channels[4]: integer(1);
}

// 定义：变体 Status
variant Status
{
    Idle;
    Loading: integer(4);
    Done;
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：以共同体成员构造
    local pixel: Pixel = Pixel.RGBA(Color(1, 2, 3, 4));

    // 合法：成员构造与堆分配组合
    heap heapPixel: Pixel = Pixel.RGBA(Color(1, 2, 3, 4));
    dispose heapPixel;

    // 合法：以携带载荷的变体状态构造
    local status: Status = Status.Loading(50);

    // 合法：无载荷状态表现为成员值，不经构造
    local done: Status = Status.Done;

    return 0;
}

// 非法：无载荷状态经成员构造表达式构造
// local bad: Status = Status.Done();
// 错误：Done 不携带载荷，不得经成员构造表达式构造

// 非法：实参类型与成员类型不匹配
// local bad: Pixel = Pixel.RGBA(1);
// 错误：RGBA 的类型为 Color

// 非法：成员不存在
// local bad: Pixel = Pixel.Unknown(Color(1, 2, 3, 4));
// 错误：Unknown 不是 Pixel 的成员
```

## 4.8 is 表达式  (spec line 4271)

```novasharp
// 定义：类 Animal
abstract class Animal
{
}

// 定义：类 Cat
class Cat: Animal
{
}

// 定义：接口 IComparable
interface IComparable
{
}

// 定义：变体 Status
variant Status
{
    Idle;
    Loading: integer(4);
    Done;
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    local animal: Animal = Cat();

    // 合法：类类型判定
    local isCat: boolean = animal is Cat; // true

    // 合法：接口判定
    local comparable: boolean = animal is IComparable; // false

    // 合法：变体成员判定
    local status: Status = Status.Loading(50);
    local loading: boolean = status is Status.Loading; // true

    // 合法：非数判定
    local ratio: float(4) = illegal;
    local bad: boolean = ratio is illegal; // true

    // 合法：无穷判定
    local upper: float(4) = infinite;
    local positive: boolean = upper is infinite; // true
    local negative: boolean = -infinite is -infinite; // true
    local mixed: boolean = upper is -infinite; // false

    // 合法：静态上不可能成立，恒假
    local impossible: boolean = 1 is Cat; // false

    return 0;
}

// 非法：判定目标不是类类型、接口或变体成员
// local bad: boolean = 1 is integer(4);
// 错误：is 表达式的形态标识符必须指向类类型、接口或变体成员

// 非法：以非浮点类型判定非数
// local bad: boolean = 1 is illegal;
// 错误：非法值字面量的左操作数类型必须为浮点类型
```

## 4.9 运算符  (spec line 4435)

```novasharp
// 定义：结构体 Vector2
structure Vector2
{
    public X: float(4);
    public Y: float(4);

    extension
    {
        public fixed Construct(this: &Vector2, x: float(4), y: float(4))
        {
            this.X = x;
            this.Y = y;
        }

        // 定义：加法重载
        // 合法：二元加法重载
        operator +(left: Vector2, right: Vector2): Vector2
        {
            return Vector2(left.X + right.X, left.Y + right.Y);
        }
    }
}

// 定义：结构体 Buffer
structure Buffer
{
    private Items[8]: integer(4);
    private Count: integer(4);

    extension
    {
        // 合法：索引访问器写在扩展块内
        operator [index: cardinal(4)]: integer(4)
        {
            get = (this: &Buffer, index: cardinal(4)) integer(4) => { return this.Items[index]; }
            set = (this: &Buffer, index: cardinal(4), value: integer(4)) => { this.Items[index] = value; }
        }

        // 合法：运算符也可以写在扩展块内
        operator ==(left: Buffer, right: Buffer): boolean
        {
            return left.Count == right.Count;
        }
    }
}

// 非法：参数数量不为一个或两个
// operator -: function(a: Vector2, b: Vector2, c: Vector2) Vector2 { }
// 错误：运算符类型必须包含一个或两个参数

// 非法：含 this 参数
// operator +: function(this: Vector2, right: Vector2) Vector2 { }
// 错误：运算符类型不得包含 this 参数

// 非法：符号不在可重载集合中
// operator &: function(left: Vector2, right: Vector2) Vector2 { }
// 错误：& 不可重载

// 非法：携带可访问性修饰符
// public operator +: function(left: Vector2, right: Vector2) Vector2 { }
// 错误：运算符定义不得携带可访问性修饰符

// 非法：在全局重复定义
// operator +: function(left: Vector2, right: Vector2) Vector2 { }
// 错误：运算符定义不得在全局重复

// 非法：索引访问器出现在类型声明体外
// operator [index: cardinal(4)]: integer(4) { get; set; }
// 错误：索引访问器必须出现在扩展块内
```

## 4.9.1 运算符优先级  (spec line 4556)

```novasharp
// 定义：类 Cat
class Cat
{
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：取负先于乘法
    local a: integer(4) = -2 * 3; // -6

    // 合法：乘法先于加法
    local b: integer(4) = 1 + 2 * 3; // 7

    // 合法：加法先于左移
    local c: integer(4) = 1 + 2 << 3; // (1 + 2) << 3

    // 合法：is 先于相等
    local d: boolean = 1 is Cat == false; // (1 is Cat) == false

    // 合法：括号改变求值优先级
    local e: integer(4) = (1 + 2) * 3; // 9

    return 0;
}
```

## 4.9.2 运算符重载  (spec line 4656)

```novasharp
// 定义：结构体 Vector2
structure Vector2
{
    public X: float(4);
    public Y: float(4);

    extension
    {
        public fixed Construct(this: &Vector2, x: float(4), y: float(4))
        {
            this.X = x;
            this.Y = y;
        }

        // 定义：加法重载
        operator +(left: Vector2, right: Vector2): Vector2
        {
            return Vector2(left.X + right.X, left.Y + right.Y);
        }
    }
}

// 非法：运算符定义在扩展块之外
// operator +: function(left: Vector2, right: Vector2) Vector2 { return left; }
// 错误：运算符定义必须声明在扩展块内

// 定义：函数 Main
fixed Main(): integer(4)
{
    local a: Vector2 = Vector2(1, 2);
    local b: Vector2 = Vector2(3, 4);

    // 合法：匹配加法重载
    local c: Vector2 = a + b;

    // 非法：无匹配的加法重载
    // local bad: Vector2 = a + 1;
    // 错误：没有符合左、右表达式类型的加法重载
}
```

## 4.9.3 取负表达式  (spec line 4724)

```novasharp
// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：整数字面量取负
    local negative: integer(4) = -5;

    // 合法：变量取负
    local value: integer(4) = 7;
    local flipped: integer(4) = -value; // -7

    // 合法：负无穷
    local lower: float(4) = -infinite;

    // 非法：不存在取负重载
    // local bad: boolean = -true;
    // 错误：操作数类型必须存在取负重载
}
```

## 4.9.6 引用表达式  (spec line 4814)

```novasharp
// 定义：类 Point
class Point
{
    public X: integer(4);
}

// 合法：标识符可寻址
local count: integer(4) = 0;
local refCount: pointer integer(4) = &count;

// 合法：基可寻址的成员访问可寻址
local pair: Point = Point();
local refX: pointer integer(4) = &pair.X;

// 合法：基可寻址的索引访问可寻址
local numbers[4]: integer(4) = { 1, 2, 3, 4 };
local refElement: pointer integer(4) = &numbers[0];

// 非法：临时量不可寻址
// local bad: pointer integer(4) = &(count + 1);
// 错误：引用表达式的返回值必须可寻址

// 非法：对临时量的成员取地址
// local bad: pointer integer(4) = &Point().X;
// 错误：引用表达式的返回值必须可寻址
```

## 4.9.26 空值合并表达式  (spec line 5331)

```novasharp
// Optional 的 ??：Value 取载荷；Null 取替代值
local maybe: Optional<integer(4)> = Optional<integer(4)>.Null;
local fallback: integer(4) = maybe ?? 0; // 0

// Optional.Null 没有载荷，裸 throw 非法
// local bad = maybe ?? throw;

// 显式给出抛出值；必须在 trap 或 catch 中
fixed OptionalThrow()
{
    trap
    {
        local explicitThrow = maybe ?? throw MyException();
    }
    catch error: Exception
    {
        // ...
    }
}

// Result 的 ??：Value 取载荷；Alternative 交给右侧续体

fixed read = (): Result<integer(4); Exception> =>
{
    return Result<integer(4); Exception>.Value(1);
};

fixed ReadOrPass(): Result<integer(4); Exception>
{
    trap
    {
        // Alternative 时在 ?? 处 throw 其载荷
        local a = read() ?? throw;

        // Alternative 时取 0，错误被丢弃
        local b = read() ?? 0;

        // Alternative 时从当前函数返回整个 Result
        local c = read() ?? return;
    }
    catch error: Exception
    {
        // ...
    }

    return Result<integer(4); Exception>.Value(0);
}
```

## 4.9.27 条件表达式  (spec line 5416)

```novasharp
// 定义：函数 Max
fixed Max(a: integer(4), b: integer(4)): integer(4)
{
    return a > b ? a : b;
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：求两者中的较大值
    local larger: integer(4) = Max(1, 2); // 2

    // 合法：取符号
    local sign: integer(4) = larger > 0 ? 1 : -1; // 1

    // 非法：条件操作数类型不为 boolean
    // local bad: integer(4) = 1 ? 2 : 3;
    // 错误：条件操作数的返回值类型必须为 boolean

    // 非法：两个分支无公共类型
    // local bad: integer(4) = true ? 1 : false;
    // 错误：真值分支与假值分支必须有公共类型

    return 0;
}
```

## 5.1 代码块  (spec line 5562)

```novasharp
// 定义：函数 Main 的代码块
fixed Main(): integer(4)
{
    local a: integer(4) = 1;

    return 0;
}

// 非法：缺少匹配的终止字符
// {
//     local b: integer(4) = 1;
// 错误：代码块未闭合

// 非法：缺少匹配的终止字符
// fixed Main: function() integer(4)
// {
//     local a: integer(4) = 1;
// 错误：代码块未闭合
```

## 5.2 local  (spec line 5635)

```novasharp
// 合法：定义变量
local Count: integer(4) = 15;

// 合法：自动类型推断
local AutoTypesCount = 15;

// 合法：未初始化的变量
local value: integer(4);

// 非法：既无类型也无初始值
// local bad;
// 错误：变量定义必须至少包含类型或初始值

// 非法：类型体内定义变量
// structure Bad
// {
//     public local X: integer(4);
// }
// 错误：类型体内不得定义变量，必须使用字段定义
```

## 5.2 local  (spec line 5659)

```novasharp
local a = 1, b = () => { return; }, c: boolean;
```

## 5.3 fixed  (spec line 5715)

```novasharp
// 合法：定义常量
fixed INT_MAX: integer(4) = 0xFFFF;

// 合法：自动类型推断
fixed Flag = true;

// 合法：泛型函数常量
fixed Echo<T>(value: T): T
{
    return value;
}

// 合法：脱糖的函数常量
fixed Desugar() =
    () =>
    {

    }

// 定义：接口 IValue
interface IValue
{
    // 合法：接口内的常量不得初始化
    public fixed Default: integer(4);
}

// 非法：常量缺少初始化
// fixed Bad: integer(4);
// 错误：常量必须被初始化

// 非法：接口内的常量被初始化
// interface Bad
// {
//     public fixed Value: integer(4) = 0;
// }
// 错误：接口内的常量不得被初始化
```

## 5.4 heap  (spec line 5821)

```novasharp
// 定义：类 Node
class Node
{
    public Value: integer(4);
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 假设：DefaultAllocator 已由开发者提供

    // 合法：使用默认分配器
    heap node: Node = Node();

    // 合法：显式分配器
    heap(DefaultAllocator) other: Node = Node();

    // 合法：手动堆分配
    manual heap raw: Node = Node();

    // 合法：值类型的堆分配
    heap number: integer(4) = 42;

    // 合法：以 7 覆盖实例字段
    node.Value = 7;

    // 合法：以 43 覆盖堆存储
    number = 43;

    return 0;
}

// 非法：manual 位于 heap 之后
// heap bad manual: integer(4) = 1;
// 错误：manual 必须位于 heap 之前
```

## 5.5 linkable  (spec line 5930)

```novasharp
linkable ExternalCounter: integer(4);
linkable ExternalBuffer: pointer cardinal(1);
linkable ExternalCallback: function(value: integer(4)) integer(4);

public linkable("Total") ExternalValue: integer(4);
```

## 5.6 use 语句  (spec line 5964)

```novasharp
// 定义：模块 MyModule
module MyModule
{
    local Flag: boolean = true;
}

// 定义：模块 OtherModule
module OtherModule
{
    local Count: integer(4) = 0;
}

// 合法：引入模块的成员
use MyModule, OtherModule;

// 合法：在代码块中引入，仅对该块内其后的代码生效
fixed Main(): integer(4)
{
    use MyModule;

    return 0;
}
```

## 5.7 赋值语句  (spec line 6039)

```novasharp
// 定义：类 Counter
class Counter
{
    public Count: integer(4);

    public Items[3]: integer(4);

    extension
    {
        public accessor Value: integer(4) => Count;

        operator [index: cardinal(4)]: integer(4)
        {
            get = (this: &Counter, index: cardinal(4)) integer(4) => { return this.Items[index]; }
            set = (this: &Counter, index: cardinal(4), value: integer(4)) => { this.Items[index] = value; }
        }
    }
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 定义：变量 value
    local value: integer(4) = 0;

    // 合法：以 1 覆盖 value 所占内存
    value = 1;

    // 合法：复合赋值，目标只被求值一次
    value += 1;

    // 合法：以 5 覆盖数组元素所占内存
    local numbers[3]: integer(4) = { 1, 2, 3 };
    numbers[0] = 5;

    // 合法：复合赋值作用于数组元素
    numbers[0] += 1;

    // 定义：实例 counter
    local counter: Counter = Counter();

    // 合法：以 2 覆盖字段所占内存
    counter.Count = 2;

    // 合法：以 3 覆盖访问器所占内存
    counter.Value = 3;

    // 合法：以 6 覆盖索引访问器所占内存，调用其 set
    counter[0] = 6;

    // 定义：堆分配实例 node
    heap node: Counter = Counter();

    // 合法：以 4 覆盖堆存储所占内存
    node.Count = 4;

    // 定义：常量 immutable
    fixed immutable: integer(4) = 4;

    // 非法：不可为常量赋值
    // immutable = 5
    // 错误：immutable 在当前上下文中不可被赋值

    return 0;
}
```

## 5.8 dispose 语句  (spec line 6158)

```novasharp
// 定义：类 Token
class Token
{
    public Value: integer(4);
}

// 定义：函数 Make
fixed Make(): Token
{
    // 合法：token 经返回值逃逸，本函数内不存在安全点，NOLAA 不插入 dispose
    heap token: Token = Token();

    return token;
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：销毁自动堆存储
    heap token: Token = Token();
    dispose token;

    // 合法：销毁手动堆存储
    manual heap raw: Token = Token();
    dispose raw;

    // 合法：变量指向堆分配单位，可被销毁
    local made: Token = Make();
    dispose made;

    return 0;
}

// 非法：销毁指向作用域存储期的实例
// local local: integer(4) = 1;
// dispose local;
// 错误：销毁目标必须指向堆分配单位

// 危险：销毁后再次访问
// heap token: Token = Token();
// dispose token;
// token.Value = 1;
```

## 5.9.2 if 语句  (spec line 6282)

```novasharp
// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：基本 if
    if true { }

    // 合法：else if 链
    if false { }
    else if true { }
    else { }

    // 非法：条件不为 boolean，且不存在可用的隐式转换
    // if 1 { }
    // 错误：条件表达式返回值类型必须为 boolean 或可隐式转换为 boolean

    {
        // 合法：在函数体内的代码块内，依然属于在函数体内
        if true { }
    }

    // 非法：条件表达式访问 Block 内标识符
    // if x
    // {
    //     local x: integer(4) = 1;
    // }
    // 错误：未定义的标识符 x；条件表达式不得访问 Block 中定义的标识符

    // 非法：条件表达式返回值类型必须为 boolean
    // if "text" { }
    // 错误：类型不匹配
}

// 非法：函数体外的 if 语句
// if true { }
// 错误：意外的 if 语句
```

## 5.9.3 while 语句  (spec line 6351)

```novasharp
// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：基本 while
    while true { }

    // 合法：带逻辑控制流名称
    while false : Loop { }

    // 非法：条件不为 boolean，且不存在可用的隐式转换
    // while 1 { }
    // 错误：条件表达式返回值类型必须为 boolean 或可隐式转换为 boolean

    {
        // 合法：在函数体内的代码块内，依然属于在函数体内
        while true { }
    }

    // 非法：条件表达式访问 Block 内标识符
    // while x
    // {
    //     local x: integer(4) = 1;
    // }
    // 错误：未定义的标识符 x；条件表达式不得访问 Block 中定义的标识符
}

// 非法：函数体外的 while 语句
// while true { }
// 错误：意外的 while 语句
```

## 5.9.4 do while 语句  (spec line 6414)

```novasharp
// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：基本 do while
    do { } while true;

    // 合法：条件访问 Block 内标识符
    do { local x: integer(4) = 1; } while x == 1;

    {
        // 合法：在函数体内的代码块内，依然属于在函数体内
        do { } while true;
    }
}

// 非法：函数体外的 do while 语句
// do { } while true;
// 错误：意外的 do while 语句
```

## 5.9.5 for 语句  (spec line 6525)

```novasharp
fixed Main(): integer(4)
{
    // 合法：遍历数组（拷贝）
    local numbers[5]: integer(4) = { 10, 20, 30, 40, 50 };
    for i: integer(4) in numbers { }

    // 合法：遍历数组（引用）
    for &ref: integer(4) in numbers
    {
        ref = 0; // 修改原数组元素
    }

    // 合法：基本区间
    for i: integer(4) in 0..5 { } // 0, 1, 2, 3, 4, 5

    // 合法：左闭右开区间
    for i: integer(4) in 0..!5 { } // 0, 1, 2, 3, 4

    // 合法：左闭右开区间（逆向）
    for i: integer(4) in 5..!0 { } // 5, 4, 3, 2, 1

    // 合法：以区间展开所得的数组为遍历目标，进入时求值一次并固定
    for i: integer(4) in {[0..!5]} { } // 0, 1, 2, 3, 4

    // 合法：带步长区间
    for i: integer(4) in 0..2..10 { } // 0, 2, 4, 6, 8, 10

    // 合法：反向区间
    for i: integer(4) in 10..-2..0 { } // 10, 8, 6, 4, 2, 0

    {
        // 合法：在函数体内的代码块内，依然属于在函数体内
        for i: integer(4) in 0..5 { }
    }

    // 非法：区间类型不匹配
    // for i: integer(4) in 0.."text" { }
    // 错误：区间表达式类型必须一致

    // 非法：迭代变量访问外部
    // for i: integer(4) in numbers { }
    // local x: integer(4) = i;
    // 错误：迭代变量不得在外部访问
}

// 非法：函数体外的 for 语句
// for i: integer(4) in 0..5 { }
// 错误：意外的 for 语句
```

## 5.9.6 match 语句  (spec line 6671)

```novasharp
// 定义：类 Animal
abstract class Animal
{
}

// 定义：类 Cat
class Cat: Animal
{
}

// 定义：变体 Option
variant Option
{
    Some: integer(4);
    None;
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：值相等模式
    local x: integer(4) = 5;
    match x
    {
        case 1 { }
        case 5 { }
        case wildcard { }
    }

    // 合法：is 模式，类类型判定并收窄
    local animal: Animal = Cat();
    match animal
    {
        case is cat:Cat { } // cat 为收窄为 Cat 后的值
        case wildcard { }
    }

    // 合法：is 模式，变体成员判定并绑定载荷
    local opt: Option = Option.Some(42);
    match opt
    {
        case is value:Option.Some { } // value = 42
        case is Option.None { }
    }

    // 合法：is 模式，非数判定
    local ratio: float(4) = illegal;
    match ratio
    {
        case is illegal { }
        case wildcard { }
    }

    // 合法：比较模式
    match x
    {
        case > 3 { }
        case < 0 { }
        case wildcard { }
    }

    // 合法：绑定之后的守卫条件
    match opt
    {
        case is v:Option.Some when v > 3 { }
        case is Option.Some { }
        case is Option.None { }
    }

    {
        // 合法：在函数体内的代码块内，依然属于在函数体内
        // 合法：静态上不可能成立，恒假
        match x
        {
            case is cat:Cat { } // 恒假：integer(4) 与 Cat 的判定不可能成立
            case wildcard { }
        }
    }

    return 0;
}

// 非法：判定目标不是类类型、接口或变体成员
// match opt { case is i:Option { } }
// 错误：is 模式的判定目标必须指向类类型、接口或变体成员

// 非法：不携带载荷的变体成员携带绑定
// match opt { case is value:Option.None { } }
// 错误：判定目标指向不携带载荷的变体成员时，禁止模式携带 Name

// 非法：函数体外的 match 语句
// match 1 { }
// 错误：意外的 match 语句
```

## 5.9.7 break 语句  (spec line 6791)

```novasharp
// 定义：函数 Main
fixed Main(): integer(4)
{
    while true : Outer
    {
        while true : Inner
        {
            // 合法：退出外部循环
            break Outer
        }
    }

    for i: integer(4) in 1..10
    {
        if i > 5
        {
            // 合法：退出 for 循环
            break;
        }
    }

    if 1 + 1 == 2
    {
        // 非法：没有可被 break 的逻辑控制流语句
        // break;
        // 错误：意外的 break
    }

    return 0;
}
```

## 5.9.8 continue 语句  (spec line 6853)

```novasharp
// 定义：函数 Main
fixed Main(): integer(4)
{
    for i: integer(4) in 1..10 : Outer
    {
        while true : Inner
        {
            // 合法：退出内部循环，继续外部循环
            continue Outer
        }
    }

    match 2
    {
        case > 1
        {
            // 合法：退出当前 case 并尝试继续向下匹配
            continue;
        }
        case 1
        {
            // 该 Block 不会执行：2 == 1 不成立
            // 注意：NovaSharp 不会 fall-through
        }
        case wildcard
        {
            // 该 Block 会被执行：模式匹配通过
        }
    }

    if 1 + 1 == 2
    {
        // 非法：没有可被 continue 的逻辑控制流语句
        // continue;
        // 错误：意外的 continue
    }

    return 0;
}
```

## 5.10.2 return 语句  (spec line 6969)

```novasharp
fixed ReturnNumber(): integer(4)
{
    // 合法：类型相同
    return 14;
}

// 定义：枚举 Status
enumeration Status: cardinal(2) { Idle; Running; }

fixed ReturnShort(): cardinal(2)
{
    // 合法：枚举值可单向隐式转换为其底层类型值（见 [3.7.4](#374-枚举)）
    return Status.Running;
}

fixed ReturnEmpty()
{
    // 合法：返回值类型为空类型时省略返回值表达式
    return;
}

// 示例：泛型
fixed Echo<T>(value: T): T
{
    // 合法：类型相同
    return value;
}

// 非法：存在不以 return 语句终止的控制流路径
// fixed Bad: function(flag: boolean) integer(4)
// {
//     if (flag)
//     {
//         return 1;
//     }
// }
// 错误：返回值类型不为空类型的函数必须在所有控制流路径上以 return 语句终止

// 非法：return 语句只能出现在函数体内部
// return 0;
// 错误：意外的 return 语句
```

## 5.10.3 throw 语句  (spec line 7041)

```novasharp
// 合法：throw 在 trap 块内
fixed Handle()
{
    trap
    {
        throw OutOfRangeException();
    }
    catch error: OutOfRangeException
    {
        // 拦截
    }
}

// 合法：throw 在 catch 块内，交由外层 trap
fixed Nested()
{
    trap
    {
        trap
        {
            local value: integer(4) = 1;
            throw value;
        }
        catch error: integer(4)
        {
            throw OutOfRangeException();
        }
    }
    catch error: Exception
    {
        // 拦截内层 catch 抛出的异常
    }
}

// 非法：throw 不在 trap 或 catch 块内
// throw 1;
// 错误：throw 语句必须仅出现在 trap 块或 catch 块中
```

## 5.10.4 trap 语句  (spec line 7144)

```novasharp
// 合法：throw 在 trap 体内，由 catch 拦截
fixed Handle(): integer(4)
{
    trap
    {
        throw OutOfRangeException();
    }
    catch error: OutOfRangeException
    {
        return 1;
    }

    return 0;
}

// 合法：内层 catch 不匹配，外层 catch 以基类拦截
fixed Nested(): integer(4)
{
    trap
    {
        trap
        {
            throw OutOfRangeException();
        }
        catch error: DivideByZeroException
        {
            // 不匹配
        }
    }
    catch error: Exception
    {
        return 1;
    }

    return 0;
}

// 合法：finally 在 return 之前执行
fixed WithFinally(): integer(4)
{
    trap
    {
        return 1;
    }
    finally
    {
        // 在 return 之前执行
    }
}

// 合法：wildcard 捕获所有抛出值且不绑定
trap
{
    throw OutOfRangeException();
}
catch wildcard
{
}

// 非法：trap 语句出现在函数体外
// trap { }
// catch error: Exception { }
// 错误：trap 语句必须仅出现在函数体中

// 非法：最终块包含 return
// trap { }
// finally { return; }
// 错误：最终块不得包含 return 语句
```

## 6.4 遮蔽  (spec line 7302)

```novasharp
// 定义：全局变量 a
local a: integer(4) = 1;

fixed Main(): integer(4)
{
    // 定义：局部变量 a，遮蔽全局变量 a
    local a: integer(4) = 2;

    // 合法：访问局部变量 a
    local b: integer(4) = a; // b = 2

    // 合法：使用 owner 访问被遮蔽的全局变量 a
    local c: integer(4) = owner.a; // c = 1

    {
        // 定义：代码块内的局部变量 a，遮蔽 Main 的 a
        local a: integer(8) = 3;

        // 合法：嵌套 owner
        local d: integer(4) = owner.owner.a; // d = 1
    }

    return 0;
}
```

## 7. 模块  (spec line 7359)

```novasharp
// 合法：块级模块
module MyModule
{
    local Flag: boolean = true;

    // 合法：嵌套模块
    module MySubModule
    {

    }
}

// 非法：将模块用作类型
// local bad: MyModule;
// 错误：模块不得作为类型使用
```

## 8.1 函数定义语法糖  (spec line 7421)

```novasharp
// 合法：函数常量
fixed Main: function() integer(4)
{
    return 0;
}

// 合法：函数简写，展开为上述显式形式
fixed Shorthand(): integer(4)
{
    return 0;
}

// 合法：脱糖写法
fixed Desugar: function() =
    () =>
    {

    };

// 合法：泛型函数常量
fixed Echo<T>: function(value: T) T
{
    return value;
}

// 定义：类 MyClass
class MyClass
{
    extension
    {
        // 合法：方法
        public fixed Inc: function(this: MyClass)
        {

        }

        // 合法：普通函数常量
        public fixed F: function()
        {

        }
    }

    // 合法：函数字段，属于字段定义，不使用本语法糖
    public Callback: function();
}

// 非法：省略 this 参数
// class Bad
// {
//     extension
//     {
//         public fixed Inc: function() { }
//         public fixed Wrong: function(this: Bad)
//         {
//             this.Inc();
//         }
//     }
// }
// 错误：this 参数必须显式书写

// 非法：无 Block 且无赋值
// fixed Bad: function();
// 错误：常量必须被初始化

// 非法：非函数类型使用该语法糖
// fixed Bad: integer(4) { }
// 错误：函数定义语法糖的常量类型必须为函数类型
```

## 8.2 参数  (spec line 7560)

```novasharp
// 合法：函数类型构造器，参数名可省略
local Callback(integer(4));
local Named(value: integer(4));
local Ref(&integer(4));
local NamedRef(value: &integer(4));

// 合法：编译期常量参数
local CompileTime(#integer(4));
local NamedCompileTime(#value: integer(4));

// 合法：携带默认值
local WithDefault(value: integer(4) = 0): integer(4);

// 合法：this 参数
local WithThis(this: MyClass);

// 合法：函数字面量，参数类型推断
local F(value: integer(4)): integer(4) =
    (value) integer(4) => value;

// 合法：函数字面量，引用参数类型推断
local G(value: &integer(4)): integer(4) =
    (value: &integer(4)) integer(4) => value;

// 非法：函数字面量省略参数名
// local Bad: function(integer(4)) integer(4) =
//     (integer(4)) integer(4) => 0;
// 错误：函数字面量的参数名必须出现

// 非法：函数字面量携带默认值
// local Bad: function(value: integer(4)) integer(4) =
//     (value: integer(4) = 0) integer(4) => value;
// 错误：函数字面量的参数不得携带默认值表达式
```

## 8.3 实参  (spec line 7636)

```novasharp
// 定义：函数 Echo
fixed Echo(value: integer(4)): integer(4)
{
    return value;
}

// 定义：函数 WithDefault
fixed WithDefault(value: integer(4) = 0): integer(4)
{
    return value;
}

// 合法：按顺序传入实参
local a: integer(4) = Echo(1);

// 合法：携带实参名
local b: integer(4) = Echo(value: 2);

// 合法：省略具有默认值的实参
local c: integer(4) = WithDefault();

// 非法：命名实参之后出现无名实参
// local bad: integer(4) = Echo(value: 1, 2);
// 错误：命名实参后的所有实参必须携带实参名
```

## 8.4 方法  (spec line 7704)

```novasharp
// 定义：类 Counter
class Counter
{
    public Count: integer(4);

    extension
    {
        public fixed Construct(this: Counter)
        {
            this.Count = 0;
        }

        public fixed Increment(this: Counter)
        {
            this.Count = this.Count + 1;
        }
    }
}

// 定义：结构体 ValueCounter
// 合法：值类型方法，this 参数类型前必须写 &
structure ValueCounter
{
    public Count: integer(4);

    extension
    {
        public fixed Increment(this: &ValueCounter)
        {
            this.Count = this.Count + 1;
        }
    }
}

// 合法：扩展方法，声明在扩展块内
extension for Counter
{
    public fixed Reset(this: Counter)
    {
        this.Count = 0;
    }
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    local c: Counter = Counter();
    c.Increment();
    c.Reset();

    return 0;
}

// 非法：值类型的 this 参数类型前未写 &
// structure Bad
// {
//     extension
//     {
//         public fixed Increment: function(this: Bad) { }
//     }
// }
// 错误：值类型方法的 this 参数类型前必须写 &

// 非法：静态函数常量的参数列表含有 this 参数
// class BadStatic
// {
//     extension
//     {
//         public static fixed F: function(this: BadStatic) { }
//     }
// }
// 错误：静态函数常量的参数列表不得含有 this 参数

// 非法：在扩展块之外携带 this 参数
// fixed Reset: function(this: Counter) { }
// 错误：函数常量不得在扩展块之外携带 this 参数

// 非法：以普通函数调用形式显式传入 this 参数
// Reset(c);
// 错误：方法不得以普通函数调用形式显式传入 this 参数
```

## 8.5 重载决议  (spec line 7830)

```novasharp
// 定义：函数常量 Calculate
fixed Calculate(a: integer(4), b: integer(4)): integer(4)
{
    return a + b;
}

// 定义：函数常量 Calculate
// 合法：同名但参数列表不同
fixed Calculate(a: float(4), b: float(4)): float(4)
{
    return a + b;
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：按实参类型决议到整数版本
    local i: integer(4) = Calculate(1, 2);

    // 合法：按实参类型决议到浮点版本
    local f: float(4) = Calculate(1.0, 2.0);

    // 非法：无匹配的重载
    // local bad: integer(4) = Calculate(true, false);
    // 错误：实参类型无匹配的函数常量

    return 0;
}

// 非法：仅返回值类型不同
// fixed Bad: function() integer(4) { return 0; }
// fixed Bad: function() boolean { return false; }
// 错误：函数常量之间的参数列表不得重复

// 非法：仅调用约定不同
// fixed Bad: function() { }
// fixed Bad: function(; CDeclaration) { }
// 错误：函数常量之间的参数列表不得重复

```

## 9. 修饰符  (spec line 7941)

```novasharp
// 定义：类 Singleton
// 合法：多个修饰符，顺序任意
public sealed class Singleton
{
    // 定义：常量 Instance
    // 合法：静态只读
    public static readonly Instance: Singleton;
}

// 非法：同一修饰符重复出现
// public public class Bad { }
// 错误：同一修饰符不得重复出现

// 非法：修饰符位于其目标之后
// class public Bad { }
// 错误：修饰符必须位于其目标之前
```

## 9.1 可访问性修饰符  (spec line 8022)

```novasharp
// 定义：类 Container
class Container
{
    extension
    {
        private fixed Hidden(): integer(4) { return 0; }

        // 非法：把访问权更大的函数值存入可访问范围更大的位置
        // public Exposed: function() integer(4) = Hidden;
        // 错误：承载函数值的位置的可访问范围大于该函数值的访问权
    }
}

// 定义：类 Animal
class Animal
{
    extension
    {
        // 合法：基础可访问性与裁剪的组合
        public scoped(Animal) fixed Shared()
        {

        }
    }
}
```

## 9.2 public 修饰符  (spec line 8075)

```novasharp
// 定义：类 Animal
class Animal
{
    // 合法：任意作用域均可访问
    public Name: integer(4);
}

// 定义：类 Cat
class Cat: Animal
{
    extension
    {
        public fixed Read(this: Cat): integer(4)
        {
            // 合法：访问基类的 public 成员
            return this.Name;
        }
    }
}
```

## 9.3 private 修饰符  (spec line 8125)

```novasharp
// 定义：类 Counter
class Counter
{
    // 合法：默认可访问性
    Count: integer(4);

    extension
    {
        public fixed Increment(this: Counter)
        {
            // 合法：同一类型作用域内可访问私有成员
            this.Count = this.Count + 1;
        }
    }
}

// 定义：类 Spy
class Spy
{
    extension
    {
        public fixed Peek(target: Counter): integer(4)
        {
            // 非法：类型作用域之外不可访问私有成员
            // return target.Count;
            // 错误：Count 在当前作用域中不可访问
        }
    }
}
```

## 9.4 protected 修饰符  (spec line 8183)

```novasharp
// 定义：类 Animal
class Animal
{
    // 合法：本类型与派生类型均可访问
    protected Name: integer(4);
}

// 定义：类 Cat
class Cat: Animal
{
    extension
    {
        public fixed Read(this: Cat): integer(4)
        {
            // 合法：派生作用域内可访问
            return this.Name;
        }
    }
}

// 定义：类 Stranger
class Stranger
{
    extension
    {
        public fixed Peek(target: Animal): integer(4)
        {
            // 非法：既非本类型作用域，也非派生作用域
            // return target.Name;
            // 错误：Name 在当前作用域中不可访问
        }
    }
}
```

## 9.5 scoped 修饰符  (spec line 8269)

```novasharp
// 定义：类 Animal
class Animal
{
    extension
    {
        // 合法：自身项为 Animal 及其成员，继承项为继承自 Animal 的类型及其成员
        protected scoped(Animal) fixed VisibleToAnimal()
        {

        }

        // 合法：目标为成员，继承项即其 override
        protected scoped(Speak) fixed VisibleToSpeak()
        {

        }

        // 合法：sealed 取消继承项，仅对 Speak 自身可见
        protected scoped(sealed Speak) fixed VisibleToSpeakOnly()
        {

        }

        public virtual fixed Speak(this: Animal)
        {
            // 合法：自身项使上述三个常量均可见
        }
    }
}

// 定义：类 Cat
class Cat: Animal
{
    extension
    {
        public override fixed Speak(this: Cat)
        {
            // 合法：继承项使 VisibleToAnimal 与 VisibleToSpeak 可见
            // 非法：sealed 已取消继承项
            // VisibleToSpeakOnly;
            // 错误：VisibleToSpeakOnly 仅对 Animal.Speak 可见
        }
    }
}

// 定义：结构体 A
structure A
{
    // 合法：字段对 A 的运算符与 A 自身可见
    public scoped(operator, A) Member: integer(4);

    extension
    {
        public fixed Construct(this: &A, member: integer(4))
        {
            this.Member = member;
        }

        // 合法：以 A 为参数的运算符，可访问 A.Member
        operator +(a: A, b: A): A
        {
            return A(a.Member + b.Member);
        }

        // 合法：参数列表含 A（位于第二个参数），故仍可访问 A.Member
        operator +(a: B, b: A): integer(4)
        {
            // 合法：b.Member 可访问
            return b.Member;
        }
    }
}

// 定义：结构体 B
structure B
{
    // 合法：字段对 B 的运算符与 B 自身可见
    public scoped(operator, B) Member: boolean;

    extension
    {
        public fixed Construct(this: &B, member: boolean)
        {
            this.Member = member;
        }

        // 合法：以 B 为参数的运算符，可访问 B.Member
        operator ==(a: B, b: B): boolean
        {
            // 非法：A.Member 在此处不可访问
            // local bad: A = A(1);
            // bad.Member;
            // 错误：bad.Member 仅对 A 的运算符与 A 自身可见

            return a.Member == b.Member;
        }
    }
}
```

## 9.7 implicitable 修饰符  (spec line 8410)

```novasharp
// 定义：结构体 HSLColor
structure HSLColor
{
    extension
    {
        // 定义：转换运算符
        // 合法：允许隐式调用
        implicitable operator HSLColor: function(from: RGBColor) HSLColor
        {
            // ...
        }
    }
}

// 定义：结构体 RGBColor
structure RGBColor
{
    extension
    {
        // 定义：转换运算符
        // 合法：需要显式调用
        operator RGBColor: function(from: HSLColor) RGBColor
        {
            // ...
        }
    }
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    local rgb: RGBColor = RGBColor();
    local hsl: HSLColor = HSLColor();

    // 合法：经类型转换表达式调用
    local a: HSLColor = (rgb as HSLColor);

    // 合法：经隐式转换调用
    local b: HSLColor = rgb;

    // 非法：目标转换运算符未携带 implicitable
    // local bad: RGBColor = hsl;
    // 错误：转换运算符必须经类型转换表达式调用

    return 0;
}

// 非法：修饰非转换运算符
// implicitable operator +: function(left: RGBColor, right: RGBColor) RGBColor { }
// 错误：implicitable 的修饰目标必须为转换运算符
```

## 9.8 verbatim 修饰符  (spec line 8495)

```novasharp
// 定义：函数常量 Main
fixed Main(): integer(4)
{
    // 合法：该块内的初始化式不得被折叠
    verbatim
    {
        local sum: integer(4) = 1 + 2;
    }

    return 0;
}

```

## 9.14 aligned(N) 修饰符  (spec line 8628)

```novasharp
// 合法：紧凑布局；全部字段默认按 0 字节对齐
aligned(0) structure Compact
{
    public A: integer(1);
    public B: integer(4);
}

// 合法：全部字段默认按 8 字节对齐
aligned(8) structure Aligned
{
    public A: integer(1);
    public B: integer(4);
}

// 合法：字段覆盖结构体的默认对齐
aligned(8) structure Mixed
{
    public A: integer(1);
    public aligned(1) B: integer(1);
}
```

## 9.15 static 修饰符  (spec line 8704)

```novasharp
// 定义：结构体 Vector2D
structure Vector2D
{
    public X: float(4);
    public Y: float(4);

    extension
    {
        // 合法：静态函数常量，不含 this 参数
        public static fixed GetX(v: Vector2D): float(4)
        {
            return v.X;
        }
    }

    // 合法：静态字段
    public static InstanceCount: integer(4) = 0;
}

// 定义：类 Container
// 合法：泛型类型的静态成员
class Container<T>
{
    public static Count: integer(4) = 0;
}

// 定义：静态类 CounterSingleton
// 合法：静态类
static class CounterSingleton
{
    public static Count: integer(4);

    extension
    {
        // 合法：静态构造函数
        public static fixed Construct()
        {
            CounterSingleton.Count = 0;
        }
    }
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：通过类型标识符访问
    local v: Vector2D;
    local x: float(4) = Vector2D.GetX(v);
    Container<integer(4)>.Count = 5;
    CounterSingleton.Count = CounterSingleton.Count + 1;

    return 0;
}

// 非法：static 修饰结构体声明
// static structure Bad { }
// 错误：static 必须仅修饰类、字段、不含 this 参数的常量或访问器

// 非法：static 修饰含 this 参数的常量
// class Bad
// {
//     extension
//     {
//         public static fixed F: function(this: Bad) { }
//     }
// }
// 错误：static 不得修饰含 this 参数的常量

// 非法：为静态类创建实例
// local bad: CounterSingleton = CounterSingleton();
// 错误：静态类为全局单例，不得为其创建实例

// 非法：通过实例访问静态成员
// v.GetX(v);
// 错误：不得通过实例访问静态成员
```

## 10.1 泛型参数  (spec line 8855)

```novasharp
// 定义：结构体 Box
// 合法：带约束的泛型结构体
structure Box<T: structure>
{
    public Value: T;
}

// 定义：结构体 Pair
// 合法：多参数泛型
structure Pair<T: class; U>
{
    public First: T;
    public Second: U;
}

// 定义：接口 IComparable
interface IComparable
{
}

// 定义：结构体 SortedList
// 合法：多约束泛型参数
structure SortedList<T: class, IComparable>
{
    public Value: T;
}

// 定义：结构体 BasicBox
// 合法：基本类型种类约束
structure BasicBox<T: basic>
{
    public Value: T;
}

// 定义：函数常量 Echo
// 合法：泛型函数常量
fixed Echo<T>(value: T): T
{
    return value;
}

// 合法：编译期单态化
local x: integer(4) = Echo<integer(4)>(42);

// 非法：字段携带泛型参数
// structure Bad
// {
//     public Value<T>: T;
// }
// 错误：字段不得携带泛型参数

// 非法：约束不满足
// structure Bad<T: class>
// {
//     public Value: T;
// }
// local bad: Bad<integer(4)>;
// 错误：integer(4) 不属于 class 种类
```

## 11.2 编译期执行顺序  (spec line 9035)

```novasharp
// File A.ns
#{
    local Base: integer(4) = 1;
}

// File B.ns
#{
    local Derived: integer(4) = Base + 1;
}
```

## 11.3 编译期表达式  (spec line 9083)

```novasharp
// 合法：编译期求值，结果保留到运行时
local size: cardinal(8) = #(Type.SizeOf<Vector3>());

// 非法：最终结果为 linkable 函数
// local bad(): cardinal(8) = #(Type.SizeOf<Vector3>);
```

## 11.4 编译期块  (spec line 9145)

```novasharp
#{
    local X: integer(4) = 1;
    X = 2;
}

// 合法：X 的最终值以运行时常量形式保留
local value: integer(4) = X;

// 非法：运行期不得修改编译期变量
// X = 3;
```

## 11.5 编译期常量参数  (spec line 9201)

```novasharp
// 定义：函数 Apply
fixed Apply(#value: integer(4)): integer(4)
{
    #{
        return value + 1;
    }
}

// 合法：实参为编译期常量
local result: integer(4) = Apply(1);

// 非法：实参不是编译期常量
// local bad: integer(4) = Apply(RuntimeValue());
```

## 11.5 编译期常量参数  (spec line 9219)

```novasharp
function(#integer(4), #integer(4))
```

## 11.6 条件编译  (spec line 9280)

```novasharp
// 定义：平台模块
module WindowsPlatform
{
}

module PosixPlatform
{
}

fixed UseWindows: boolean = true;

#if UseWindows
{
    use WindowsPlatform;
}
else
{
    use PosixPlatform;
}
```

## 11.7 编译期边界  (spec line 9377)

```novasharp
#{
    local X: integer(4) = 1;
    X = 2;
}

// 合法：X 的最终值成为运行时常量
local value: integer(4) = X;

// 非法：运行期不能修改编译期变量
// X = 3;

// 非法：编译期定义的类型不能进入运行时
// local bad: CompileTimeType;
```

## 12.1 内存分配  (spec line 9482)

```novasharp
// 定义：类 Cat
class Cat
{
    public Legs: integer(4);
}

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：作用域存储期
    local cat: Cat = Cat();

    // 合法：自动堆存储期
    heap heapCat: Cat = Cat();

    // 合法：手动堆存储期
    manual heap rawCat: Cat = Cat();

    return 0;
}
```

## 12.2 NOLAA  (spec line 9589)

```novasharp
// 定义：类 Token
class Token
{
    public Value: integer(4);
}

// 定义：变量 Global
local Global: Token;

// 定义：函数 Main
fixed Main(): integer(4)
{
    // 合法：NOLAA 在 token 的最后一次使用之后插入 dispose
    heap token: Token = Token();
    token.Value = 1;

    // 不推荐：需求经字段写入延续，此处销毁早于安全点
    // NOLAA 将发出 Early-Dispose Warning
    heap shared: Token = Token();
    Global = shared;
    dispose shared;

    return 0;
}

// 危险：同一分配单位被销毁两次，NOLAA 将发出 Double-Dispose Warning
// heap duplicated: Token = Token();
// dispose duplicated;
// dispose duplicated;
```

## 14.1 主函数  (spec line 9695)

```novasharp
// 合法：返回值类型为 integer(4)，由 Launcher 原样交给入口点
fixed Main(): integer(4)
{
    return 0;
}

// 合法：以平台实参为参数，由被调用方自行解释
fixed Main(handle: pointer, table: pointer): integer(4)
{
    return 0;
}

// 合法：返回值为空类型，入口点不传递任何值
fixed Main()
{
    return;
}

// 非法：一个可执行产物存在两个主函数
// fixed Main: function() integer(4) { return 1; }
// 错误：一个可执行产物必须恰有一个主函数
```

## 14.2 启动流程  (spec line 9778)

```novasharp
// 定义：函数 Main
fixed Main(): integer(4)
{
    return 0;
}

// 启动流程：
// 1. 入口点调用 Launcher
// 2. Launcher 执行全部静态构造函数与运行期初始化
// 3. Launcher 调用 Main，取得返回值 0
// 4. Launcher 将 0 原样交给入口点，入口点以此终止程序

// 非法：初始化之间的依赖关系成环，无法确定顺序
// module A { local X: integer(4) = B.Y; }
// module B { local Y: integer(4) = A.X; }
// 错误：初始化之间的依赖关系成环
```

## 15.2 Built-In Library 完整内容  (spec line 10161)

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

## 15.3 Compile-Time Library 完整内容  (spec line 10367)

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

