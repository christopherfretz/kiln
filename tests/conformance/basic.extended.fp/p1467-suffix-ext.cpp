// [lex.fcon] (P1467R9): the f32/f64/f128/bf16 floating-literal suffixes are
// C++23; kiln accepts them in earlier modes as an extension (GCC 14 does too:
// libstdc++ spells them in every mode), with a -Wc++23-extensions warning
// under -pedantic. f16 (a clang extension in every mode) is not diagnosed.
//@ expect: compile-ok
//@ std: c++20
//@ args: -pedantic
//@ strict-diags
//@ diag: 13:13 warning "'f32' suffix on a floating literal is a C++23 extension"
//@ diag: 14:13 warning "'BF16' suffix on a floating literal is a C++23 extension"
//@ diag: 15:15 warning "'F128' suffix on a floating literal is a C++23 extension"
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (no f32/bf16/f128 suffixes)
auto a = 1.5f32;
auto b = 1.5BF16;
auto c = 0x1p3F128;
auto d = 1.5f16;
