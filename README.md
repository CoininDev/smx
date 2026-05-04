# smx
## What it is:
smx is a functional programming language with first-class environments and an operator-centric model.

I am using the code from [simple_math](https://github.com/CoininDev/simple_math), a simple calculator interpreter, as a base.

Funcional paradigm, focused in operators for flow control, eager evaluation in default, and first class environments!!!

---

## Variables

Variables are defined with `=` and terminated with `;`.

```smx
a = 1;
result = a + 3;
```

`result` holds the final output. Every other variable is just part of the environment.

---

## Lists

Lists are created with the `,` operator.

```smx
list = 1, 2, 3, 4, 5;
```

> The `,` operator constructs a cons-style list. The first element is the **head**, the rest is the **tail**.

---

## Functions

Functions are defined using lambda syntax with `\pattern.`:

```smx
add = \a. \b. a + b;
result = add 5 6; // result: 11
```

Functions are curried by default — applying a function to fewer arguments than it expects returns a new function.

---

## Flow Operators

### Conditional: `?`

The `?` operator takes a boolean on the left and a list on the right. It returns the **head** if `true`, or the **tail** if `false`.

```smx
is_even = \a. a % 2 == 0 ? "yes", "no";
result = is_even 4; // result: "yes"

some_list = false ? 1, 2, 3, 4, 5; // some_list = 2, 3, 4, 5;
```

### Pipe: `:`

The `:` operator applies a value (left) to a function (right), enabling function chaining.

```smx
flux = 1, 2, 3, 4, nil : sum : double : (\x. x + 61) : to_ascii;
```

This pipes the list through `sum`, then `double`, then an anonymous function, then `to_ascii`.

OBS: Those functions does not exists yet and needs to be implemented, but I intend to implement them in stdlib.math;

---

## Frozen (Lazy) Values

By default, SMX evaluates everything eagerly. The `'` (quote/freeze) operator defers evaluation, creating a **frozen** (lazy) expression.

```smx
frozen_value = '5;
frozen_expr  = '(a + 15);
```

A frozen expression is only evaluated when explicitly forced with `eval`.

## Patterns

You can also create and use patterns as values, this is mainly for match usability but it is still in development

```smx
pat = #5;
pat = #'a; // The variables are evaluated before, so if you want it to be just a name, you need to freeze it (')

pat = #('a, 2, 3, 'b, nil) // You can use lists with '(' and ')'
pat = #'_ // _ (wildcard) is also a name, so it needs to be freezed as well

pat = #'(a ~ string) # typed names needs to be frozen too.

pat = #'(a ~ [string | number | bool])

pat = #'(a ~ {var1 ~ string; var2 ~ number;})

pat = #'(a ~ [{env1;} | {env2 ~ number;} | string])
```

### Tail-Call Recursion Example

```smx
tail_call = \self. \a. eval ( a == 0 ? 'true, '(self (a - 1)) );
```

Because the recursive branch is frozen, evaluation is deferred until needed — enabling safe tail-call patterns.

---

## Builtins
We have builtin utilities. For example:

#### try
Returns nil when an error occurs instead of canceling.

#### eval
Forces the execution of a freezed expression.

#### has
Checks if a name exists in an environment

#### zip_env
Constructs a new environment from a pattern and a value.
It supports lists.

#### head and tail
Self explanatory.


#### convert
Convert different types of values.
```smx
convert (1, 'number); // 1
convert (1.0, 'string); // "1.0"
convert ("1", 'number); // 1
convert (1, 'bool); // true
convert ("hello", 'number); // {nan = true;}
convert (1, 'u256); // 1u256
//equivalent to:
u256 1
```

### Strict Numeric Types

SMX supports fixed-size numeric types for specialized applications (e.g., smart contracts):
- **Unsigned**: `u8`, `u16`, `u32`, `u64`, `u128`, `u256`
- **Signed**: `i8`, `i16`, `i32`, `i64`, `i128`, `i256`
- **Float**: `f8`, `f16`, `f32`, `f64`, `f128`, `f256`

Declare them using suffixes or the `convert` builtin:
```smx
val = 100u256;
count = convert(val, 'i32);
or = i32 val
```
---

## Environments

An **environment** is a named scope containing variable bindings. SMX code itself is an environment. You can create and use them explicitly.

> OBS: You can define resources and operators inside environments, but they won't be accessible with a `.`

### Environment Arithmetic

Environments support arithmetic operations for merging and manipulating scopes:
- **Addition (`+`)**: Merges two environments. If keys overlap, the right-side value takes precedence.
- **Subtraction (`-`)**: Removes keys from an environment. Accepts a list of names (frozen idents or strings) or a single name.

```smx
env1 = {a = 1; b = 2;};
env2 = {b = 3; c = 4;};

merged = env1 + env2; // { a = 1; b = 3; c = 4; }
cleaned = merged - ('b, 'c, nil); // { a = 1; }
```

Important: Unlike Lisp, SMX environment haves no layers, it is a flat name => value correspondency.

### Inline Environment + Eval

Use `{}` to define an environment and `::` to evaluate an expression inside it:

```smx
{a = 1; b = 3 + 3;} :: '(ok + a);
```

### Named Environment + Member Access

```smx
env = {a = 1;};
result = env.a;
-- result: 1
```

Access environment members using `.`.

---

## Pattern Matching & Type Annotations

Variables and function parameters support **pattern matching** and **type annotations** using `~`.

### Destructuring

```smx
a ~ number, b ~ number = 1, 2;
```

### Named Environment Destructuring

```smx
num1, num2 = 1, 2;
```

### Typed Function Parameters

```smx
func = \a ~string, b ~bool. b ? a, "";
```

### Typed List Variables

```smx
listas ~ [string | number | nil] = "hello", 42, 3.14, nil;
```

The type annotation `[string | number | nil]` means the list may contain values of any of those types, in any order.

### Environment Schemas

You can enforce structure on environments using schemas:
- `var;` — requires the variable to exist.
- `var ~ type;` — requires the variable to exist and match the type.

```smx
type person = {
    name ~ string;
    age ~ number;
    metadata; // any type
};

greet = \p ~ person. "Hello, " + p.name;
```

### Type Aliases

Use the `type` keyword to define reusable type abbreviations:

```smx
type u256_list = [u256];
type point = { x ~ number; y ~ number; };
```

---

## Custom Operators

You can define your own infix or prefix operators with `op`:

```smx
op left 3 (a ++ b) = (a + b) * (a + b);
op nonassoc 4 (!!x) = eval x;
result = !!'(1 ++ 2);
-- result: 9
```

- `left` — associativity (`left` or `right`)
- `3` — precedence level
- `(a ++ b)` — operator pattern with its operands

---

## Resources

**Resources** are variables that must be explicitly imported by any definition that uses them. This makes dependencies transparent and avoids hidden global state.

```smx
resource MyNumber = 3;

sum @{MyNumber} = MyNumber + 4;
-- sum: 7
```

Resources can also be environments:

```smx
resource Example = {
    add = \a. a + a;
    sub = \a. a - a;
};

result @{Example, MyNumber} = Example.add MyNumber;
-- result: 6
```

The `@{...}` annotation declares which resources a definition requires.

There are 2 ways of requesting resources:
1: In the pattern
```smx
my_fun @{Res} = \x ~ number. Res + x;
result @{Res} = my_fun 3;
```

2: In the lambda itself
```smx
\x ~ number @{Res}. Res + x;
```
---

## The IO Resource

`IO` is a built-in resource provided by the interpreter. It handles all side effects — printing, filesystem access, and importing other files.

```smx
say_hello @{IO} = IO.print "hello world!";

get_file @{IO} = IO.fs.read "example.txt";

_ @{IO} = IO.import {
    file = "example.amb";
    skip_underscored = true;
};
```

- `IO.print` — outputs a value
- `IO.fs.read` — reads a file, returning a string
- `IO.import` — imports an `.amb` ambient file into the current environment
- `_` — a discard variable; the result is thrown away

---
## Syntactic sugars
To make the code easier to read, I've implemented some basic syntactic sugars.
### `let ... in ...`
this:
```smx
result = 
    let
        a = 1;
        b = a + 1;
        c = b + 2;
    in 
        a + b + c
;
```
is equivalent to this:
```smx
result = 
    {
        a = 1;
        b = a + 1;
        c = b + 2;
    }::'(
        a + b + c
    );
```
It creates an environment, and then applies it to the expression.

### `if ... then ... else ...`
this:
```smx
a = 3;
result = 
    if a == 4
    then 3
    else a + 1;
```
is equivalent to this:
```smx
a = "hello";
result = eval (
        a == 4 ?
        '(3),
        '(a + 1)
    );
```
this means that, `then ...` and `else ...` expressions are automatically frozen, and only the selected expression is really evaluated, this makes it safe in tail-call functions.

---

## REPL
For convenience, the REPL (accessed by -i flag) already comes with IO and support both assignment and pure expressions, but that's not true in files, while writing a file you should always use an assignment, even an unused one: 
```smx
_ = do_something 3;
_ @{IO} = IO.print "done.";
```

## File Extensions

| Extension | Purpose |
|-----------|---------|
| `.smx` | Executable file — must have a `result` entry point |
| `.amx` | Ambient file — an environment meant to be imported, no entry point |

An **Ambient** (`.amx`) is the combination of an environment (variable bindings), resources, and custom operators — a self-contained module for reuse.

---

## Quick Reference

| Feature | Syntax |
|---------|--------|
| Variable | `x = expr;` |
| List | `1, 2, 3` |
| Lambda | `\a. expr` |
| Application | `f x` |
| Conditional | `cond ? true_val, false_val` |
| Pipe | `val : func` |
| Freeze | `'expr` |
| Eval | `eval expr` |
| Environment | `{a = 1; b = 2;}` |
| Env eval | `env :: 'expr` |
| Member access | `env.name` |
| Custom operator | `op left N (a ** b) = ...` |
| Resource decl | `resource Name = expr;` |
| Resource use | `var @{Name} = ...;` |
| Type annotation | `(x ~ type)` |
| List type | `~ [t1 \| t2]` |

