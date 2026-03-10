# Bengal Language Reference

Bengal is a statically typed scripting language designed for simplicity and productivity. This document describes the
complete syntax and core features of the language.

---

## Table of Contents

1. [Basic Syntax](#basic-syntax)
2. [Comments](#comments)
3. [Variables and Constants](#variables-and-constants)
4. [Data Types](#data-types)
5. [Operators](#operators)
6. [Control Flow](#control-flow)
7. [Functions](#functions)
8. [Classes and Objects](#classes-and-objects)
9. [Interfaces](#interfaces)
10. [Enums](#enums)
11. [Type Aliases](#type-aliases)
12. [Generics](#generics)
13. [Modules and Imports](#modules-and-imports)
14. [Exception Handling](#exception-handling)
15. [Async/Await](#asyncawait)
16. [String Interpolation](#string-interpolation)
17. [Multiline Strings](#multiline-strings)
18. [Reflection](#reflection)
19. [Native Classes and FFI](#native-classes-and-ffi)

---

## Basic Syntax

Bengal uses a C-like syntax with optional semicolons. Statements are typically separated by newlines.

```bengal
// Simple variable declaration
let x = 42
let y = 10

// Function call
println("Hello, World!")

// Semicolons are optional
let a = 1; let b = 2;
```

### Shebang

Scripts can start with a shebang for direct execution:

```bengal
#!/usr/bin/env bengal

import std::io
println("Shebang works!")
```

---

## Comments

### Line Comments

Single-line comments start with `//`:

```bengal
let x = 10  // This is a line comment
```

### Block Comments

Multi-line block comments use `/* */` and support nesting:

```bengal
/* This is a block comment */

let y = 20  /* inline block comment */

/*
 * Multi-line block comment
 * with multiple lines
 */

/* Nested /* block */ comment */
```

---

## Variables and Constants

### Variable Declaration

Use `let` to declare variables:

```bengal
let x = 42
let name = "Bengal"
let active = true
```

### Type Annotations

Variables can have explicit type annotations:

```bengal
let x: int = 42
let pi: float = 3.14159
let greeting: str = "Hello"
```

### Optional Types

Types can be optional using the `?` suffix:

```bengal
let maybeValue: str? = null
let result: int? = getValue()
```

---

## Data Types

### Primitive Types

| Type    | Description           | Example           |
|---------|-----------------------|-------------------|
| `int`   | 64-bit integer        | `let x = 42`      |
| `float` | 64-bit floating point | `let pi = 3.14`   |
| `str`   | String                | `let s = "hello"` |
| `bool`  | Boolean               | `let b = true`    |

### Numeric Types

Bengal supports various numeric types for FFI and low-level operations:

| Type      | Description             | Range              |
|-----------|-------------------------|--------------------|
| `int8`    | 8-bit signed integer    | -128 to 127        |
| `uint8`   | 8-bit unsigned integer  | 0 to 255           |
| `int16`   | 16-bit signed integer   | -32,768 to 32,767  |
| `uint16`  | 16-bit unsigned integer | 0 to 65,535        |
| `int32`   | 32-bit signed integer   | -2³¹ to 2³¹-1      |
| `uint32`  | 32-bit unsigned integer | 0 to 2³²-1         |
| `int64`   | 64-bit signed integer   | -2⁶³ to 2⁶³-1      |
| `uint64`  | 64-bit unsigned integer | 0 to 2⁶⁴-1         |
| `float32` | 32-bit floating point   | ~7 decimal digits  |
| `float64` | 64-bit floating point   | ~15 decimal digits |

```bengal
let b: int8 = int8(127)
let u: uint8 = uint8(255)
let i: int16 = int16(32767)
let f: float32 = float32(3.14)
```

### Arrays

Arrays are homogeneous collections:

```bengal
let arr = [1, 2, 3]
let first = arr[0]

// Array type syntax
let numbers: int[] = [1, 2, 3]
```

### Null

`null` represents the absence of a value:

```bengal
let empty: str? = null
```

---

## Operators

### Arithmetic Operators

```bengal
let sum = 10 + 5      // Addition
let diff = 10 - 5     // Subtraction
let prod = 10 * 5     // Multiplication
let quot = 10 / 5     // Division
let mod = 10 % 5      // Modulo
```

### Increment/Decrement

```bengal
// Postfix
let a = 5
a++    // Returns 5, then increments to 6
a--    // Returns 6, then decrements to 5

// Prefix
let b = 10
++b    // Increments to 11, then returns 11
--b    // Decrements to 9, then returns 9
```

### Comparison Operators

```bengal
let eq = (x == y)     // Equal
let neq = (x != y)    // Not equal
let gt = (x > y)      // Greater than
let lt = (x < y)      // Less than
let gte = (x >= y)    // Greater than or equal
let lte = (x <= y)    // Less than or equal
```

### Logical Operators

```bengal
let and = (a && b)    // Logical AND
let or = (a || b)     // Logical OR
let not = !a          // Logical NOT
```

### Range Operator

```bengal
// Inclusive range for loops
for (i in 1..5) {
    println(str(i))
}
```

---

## Control Flow

### If/Else

```bengal
if (x > 0) {
    println("x is positive")
} else if (x < 0) {
    println("x is negative")
} else {
    println("x is zero")
}
```

### For Loops

```bengal
// Range-based for loop
for (i in 1..5) {
    println(str(i))
}

// Array iteration
let arr = [1, 2, 3]
for (item in arr) {
    println(str(item))
}
```

### While Loops

```bengal
let count = 3
while (count > 0) {
    println(str(count))
    count--
}

// Infinite loop with break
while (true) {
    if (count-- == 0) {
        break
    }
}
```

### Break and Continue

```bengal
// Break
for (i in 1..10) {
    if (i == 5) {
        break
    }
}

// Continue
for (i in 1..5) {
    if (i == 3) {
        continue  // Skip iteration
    }
    println(str(i))
}
```

---

## Functions

### Basic Functions

```bengal
fn sum(x: int, y: int): int {
    return x + y
}

fn greet(name: str) {
    println("Hello, " + name)
}
```

### Default Parameters

```bengal
fn greet(name: str, greeting: str = "Hello") {
    println(greeting + ", " + name)
}
```

### Optional Return Types

```bengal
fn findValue(key: str): str? {
    if (hasKey(key)) {
        return getValue(key)
    }
    return null
}
```

### Function Calls

```bengal
let result = sum(10, 20)
let value = findValue("key")
```

---

## Classes and Objects

### Basic Class Definition

```bengal
class Person {
    name: str = "Unknown"
    age: int = 0

    fn introduce() {
        println("I am " + self.name)
    }
}

let person = Person()
person.introduce()
```

### Constructor

```bengal
class Rectangle {
    width: float
    height: float

    constructor(w: float, h: float) {
        self.width = w
        self.height = h
    }

    fn area(): float {
        return self.width * self.height
    }
}

let rect = Rectangle(10.0, 5.0)
```

### Fields with Default Values

```bengal
class SomeObject {
    some_int: int = 10
    some_float: float = 5.0
    some_string: str = "Default string"
}
```

### Private Members

```bengal
class Counter {
    private count: int = 0

    fn increment() {
        self.count = self.count + 1
    }

    fn getCount(): int {
        return self.count
    }
}
```

### Dynamic Fields

Fields can be added dynamically at runtime:

```bengal
class SomeObject {
    some_string: str = "Default"

    fn someMethod(): str? {
        self.some_undefined = true  // Dynamic field
        return some_string
    }
}
```

### The `self` Keyword

```bengal
class Point {
    x: float = 0.0
    y: float = 0.0

    fn move(xOffset: float, yOffset: float) {
        self.x = self.x + xOffset
        self.y = self.y + yOffset
    }
}
```

---

## Interfaces

### Basic Interface

```bengal
interface Printable {
    fn print(text: str)
    fn log(text: str) {
        // Default implementation
    }
}
```

### Interface Inheritance

```bengal
interface Drawable : Printable {
    fn draw(text: str)
}

class Circle : Drawable {
    fn print(text: str) {
        // Implementation required
    }

    fn draw(text: str) {
        // Implementation required
    }
}
```

---

## Enums

### Basic Enum

```bengal
enum Status {
    Pending
    Active
    Completed
}

let status = Status.Pending
```

### Enum with Values

```bengal
enum HttpMethod {
    GET = 0
    POST = 1
    PUT = 2
    DELETE = 3
    PATCH = 4
    HEAD = 5
    OPTIONS = 6
}
```

---

## Type Aliases

```bengal
type vec2 = tvec2<float>
type ivec2 = tvec2<int>
type vec3 = tvec3<float>
type ivec3 = tvec3<int>
```

---

## Generics

### Generic Classes

```bengal
class tvec2<T> {
    x: T
    y: T
}

class tvec3<T> {
    x: T
    y: T
    z: T
}

// Usage
let v2 = tvec2<float>()
let iv2 = tvec2<int>()
```

### Generic Type Constraints

Generic types can be used with any type parameter:

```bengal
class Container<T> {
    value: T

    constructor(val: T) {
        self.value = val
    }

    fn get(): T {
        return self.value
    }
}
```

---

## Modules and Imports

### Module Declaration

```bengal
module std::fs

class FileInfo {
    is_file: bool = false
    is_dir: bool = false
    size: int = 0
}
```

### Importing Modules

```bengal
import std::io
import std::fs
import std::shell

// Use imported functions
println("Hello")
let content = fs::readString("file.txt")
```

### Using Module Members

```bengal
import std::io

// Direct call
println("Hello")

// Qualified call
std::io::println("Hello")
```

---

## Exception Handling

### Try/Catch

```bengal
try {
    println("Entering try block")
    throw "Exception!"
    println("This should not be reached")
} catch (e) {
    println("Caught: " + e)
}

println("Continuing after try-catch")
```

### Nested Try/Catch

```bengal
try {
    println("Outer try")
    try {
        println("Inner try")
        throw "Inner Exception"
    } catch (e) {
        println("Caught in inner: " + e)
        throw "Re-thrown: " + e
    }
} catch (e) {
    println("Caught in outer: " + e)
}
```

### Throw

```bengal
throw "Error message"
throw "Custom error: " + details
```

---

## Async/Await

### Async Functions

```bengal
async fn fetchData(url: str): str? {
    let response = await HttpClient::get(url)
    return response.body
}
```

### Await

```bengal
async fn main() {
    let client = HttpClient()
    let result = await client.get("https://api.example.com/data")
    println(str(result))
}

main()
```

### Async Native Functions

```bengal
async native fn sleep(ms: int)
async native fn readLine(): str
```

---

## String Interpolation

Strings can contain interpolated expressions using `${}`:

```bengal
let name = "Bengal"
let version = 1.0

println("Welcome to ${name}!")
println("Version: ${version}")
println("Result: ${10 + 20}")

// In function calls
println("Call 1: ${some_object.someMethod()}")
```

---

## Multiline Strings

Multiline strings use triple quotes `"""` and automatically handle indentation:

```bengal
let name = "Bengal"
let version = 1.0

let multi = """
    Welcome to ${name}!
    Version: ${version}

    This is a multiline string
    with indentation that should be stripped.
"""

println(multi)

// Single line multiline string
let no_strip = """one line"""

// First line stripping
let first_line_stripped = """
  first line was empty
  so this is the new first line
"""
```

### Features

- Leading/trailing empty lines are removed
- Common indentation is stripped from all lines
- String interpolation works inside multiline strings

---

## Reflection

Bengal provides runtime reflection capabilities:

```bengal
import std::reflect

class User {
    name: str = "Alice"
    age: int = 30
}

let u = User()

// Get type information
let type = std::reflect::type_of(u)      // "User"
let className = std::reflect::class_name(u)  // "User"

// Get fields as an object
let fields = std::reflect::fields(u)
println("fields.name: ${fields.name}")   // "Alice"

// JSON serialization
let jsonStr = std::json::stringify(u)
let parsed = std::json::parse(jsonStr)
println("Parsed name: ${parsed.name}")
```

### Reflection Functions

| Function                          | Description                                     |
|-----------------------------------|-------------------------------------------------|
| `std::reflect::type_of(value)`    | Returns the type name as a string               |
| `std::reflect::class_name(value)` | Returns the class name (or null for primitives) |
| `std::reflect::fields(value)`     | Returns an object with all field values         |

---

## Native Classes and FFI

### Native Class Declaration

```bengal
native class ByteBuffer {
    constructor()
    constructor(size: int)

    fn reserve(size: int)
    fn get(index: int): uint8
    fn set(index: int, value: uint8)
    fn length(): int
}
```

### Native Functions

```bengal
native fn read(path: str): Array?
native fn write(path: str, data: Array)
native fn exists(path: str): bool
```

### Native Class with Implementation

```bengal
native class MyBuffer {
    constructor(size: int)
    fn reserve(size: int)
    fn set(idx: int, val: uint8)
    fn get(idx: int): uint8
    fn length(): int

    // Can have regular method implementations
    fn fill(val: uint8) {
        for (i in 0..length()-1) {
            set(i, val)
        }
    }
}
```

---

## Type Casting

### Built-in Cast Functions

```bengal
// To int
let x = int(42.7)        // 42
let y = int("123")       // 123
let z = int(true)        // 1

// To float
let a = float(100)       // 100.0
let b = float("3.14")    // 3.14

// To string
let s = str(42)          // "42"

// To bool
let p = bool(1)          // true
let q = bool(0)          // false
let r = bool("")         // false
```

---

## Standard Library Overview

### std::io

```bengal
import std::io

fn print(text: str)
fn println(line: str)
async fn sleep(ms: int)
async fn readLine(): str
```

### std::fs

```bengal
import std::fs

fn readString(path: str): str?
fn writeString(path: str, content: str)
fn exists(path: str): bool
fn isFile(path: str): bool
fn isDir(path: str): bool
fn remove(path: str)
fn copy(from: str, to: str)
fn rename(from: str, to: str)
fn stat(path: str): FileInfo?
```

### std::shell

```bengal
import std::shell

fn sh(cmd: str): str
fn exec(cmd: str, args: str[] = []): int?
fn cd(dir: str)
```

### std::http

```bengal
import std::http

class HttpClient {
    fn setBaseUrl(url: str)
    fn setTimeout(ms: int)
    async fn get(url: str): Response?
    async fn post(url: str, body: str): Response?
}
```

### std::args

```bengal
import std::args

fn get(): str[]
fn count(): int
fn at(index: int): str?
fn program(): str
fn hasFlag(flag: str): bool
fn getFlag(flag: str): str?
```

### std::sys

```bengal
import std::sys

fn exit(code: int)
fn env(key: str): str?
fn setPwd(dir: str)

class Process {
    fn start(cmd: str, args: str[] = [], ...)
    fn wait()
    fn exitCode(): int?
    fn getStdout(): str
    fn getStderr(): str
}
```

### std::data

```bengal
import std::data

class ByteBuffer {
    constructor(size: int)
    fn get(index: int): uint8
    fn set(index: int, value: uint8)
    fn length(): int
}
```

### std::math

```bengal
import std::math

// Generic vector types
class tvec2<T>
class tvec3<T>

// Type aliases
type vec2 = tvec2<float>
type ivec2 = tvec2<int>
type vec3 = tvec3<float>
type ivec3 = tvec3<int>
```

---

## Grammar Summary

```
program     ::= shebang? statement*

shebang     ::= "#!" "/" "!" "env" identifier newline

statement   ::= import | module | class | interface | enum
              | function | type_alias | variable | expression
              | if | for | while | return | try | throw

import      ::= "import" module_path
module      ::= "module" module_path

class       ::= "class" identifier type_params? ":" interface_list?
                "{" field* method* "}"

interface   ::= "interface" identifier type_params? ":" interface_list?
                "{" method* "}"

enum        ::= "enum" identifier "{" variant* "}"

function    ::= ("async" | "native" | "async" "native")?
                "fn" identifier "(" params? ")" return_type? block

variable    ::= "let" identifier ":" type? "=" expression

if          ::= "if" "(" expression ")" block ("else" block)?

for         ::= "for" "(" identifier "in" expression ")" block
while       ::= "while" "(" expression ")" block

try         ::= "try" block "catch" "(" identifier ")" block

expression  ::= literal | identifier | function_call | method_call
              | binary_op | unary_op | cast | array | object_creation

literal     ::= string | number | "true" | "false" | "null"

string      ::= '"' character* '"' | '"""' multiline '"""'
number      ::= integer | float
integer     ::= digit+
float       ::= digit+ "." digit+

type        ::= primitive | identifier | type "?" | type "[]"
              | identifier "<" type_list ">"

primitive   ::= "int" | "float" | "str" | "bool"
              | "int8" | "uint8" | "int16" | "uint16"
              | "int32" | "uint32" | "int64" | "uint64"
              | "float32" | "float64"
```

---

## Quick Reference

### Keywords

```
import      module      class       interface   enum
fn          type        let         if          else
for         while       in          return      private
null        native      async       await       try
catch       throw       break       continue    constructor
```

### Primitive Types

```
int         float       str         bool
int8        uint8       int16       uint16
int32       uint32      int64       uint64
float32     float64
```

### Special Symbols

```
::          ->          =>          ..
??          ??=         ${}         self
```

---

## Examples

### Hello World

```bengal
import std::io

println("Hello, World!")
```

### Simple Calculator

```bengal
fn sum(x: int, y: int): int {
    return x + y
}

fn mul(x: int, y: int): int {
    return x * y
}

fn calculate(): int {
    return mul(sum(10, 20), 2)
}

print("Result: ${calculate()}")
```

### File Operations

```bengal
import std::fs

let path = "test.txt"
let content = "Hello, Bengal!"

fs::writeString(path, content)

if (fs::exists(path)) {
    let read = fs::readString(path)
    println("Content: ${read}")
    fs::remove(path)
}
```

### Command Execution

```bengal
import std::shell

let output = sh("ls -la")
println(output)

let exitCode = exec("gcc", ["-o", "main", "main.c"])
```

---

## License

Bengal is released under the MIT License.
