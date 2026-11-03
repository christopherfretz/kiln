// [expr.const]/5.15: a cast to or from a function pointer (a C-style
// cast or a reinterpret_cast) is not a core constant expression, so a
// constexpr or constinit initializer with one is an error, but clang's
// fold (a file-scope array bound, `__builtin_constant_p`) notes the cast
// and continues keeping the function: the pointer compares equal to the
// function (`h`), and a call through it folds when the call's function
// type is the function's, ignoring the exception specification (`e`,
// `f`), while `__builtin_constant_p` of it is false (spec/oven.md
// "Pointers that point at nothing"). Same output at -fclang-compat=19.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 33:20 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 34:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 34:20 note "reinterpret_cast is not allowed in a constant expression"
//@ diag: 35:18 error "variable does not have a constant initializer"
//@ diag: 35:1 note "required by 'constinit' specifier here"
//@ diag: 35:25 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 36:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 36:34 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 37:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 37:22 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 37:5 warning "variable length array folded to constant array as an extension"
//@ diag: 38:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 38:8 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 38:5 warning "variable length array folded to constant array as an extension"
//@ diag: 39:7 warning "variable length arrays in C++ are a Clang extension"
//@ diag: 39:7 note "cast that performs the conversions of a reinterpret_cast is not allowed in a constant expression"
//@ diag: 39:5 warning "variable length array folded to constant array as an extension"
constexpr int cmk(int v) { return v + 1; }
int mk(int);
constexpr auto a = (void (*)())&mk;
constexpr auto b = reinterpret_cast<void (*)(long)>(&mk);
constinit void (*c)() = (void (*)())&mk;
constexpr int d = ((int (*)(int))(void (*)())&cmk)(1);
int e[((int (*)(int))(void (*)())&cmk)(1)];
int f[((int (*)(int) noexcept)&cmk)(2)];
int h[(void (*)())&mk == (void (*)())mk];
static_assert(sizeof(e) == 2 * sizeof(int) && sizeof(f) == 3 * sizeof(int));
static_assert(sizeof(h) == sizeof(int));
static_assert(!__builtin_constant_p((void (*)())&mk));
