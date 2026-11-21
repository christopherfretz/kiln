// [diff.cpp23.lex]/1, [lex.string.uneval]: before C++26 an encoding prefix
// on an unevaluated string literal (an attribute's string argument, a
// static_assert message, a language linkage) is valid; clang warns that it
// has no effect and is incompatible with C++2c
// (-Winvalid-unevaluated-string), once per prefixed piece at its prefix,
// and uses the string as an ordinary one. C++26 makes it an error
// (unevaluated-string-macro.cpp). spec/lex.md, WP-1805.
//@ expect: compile-ok
//@ args: -std=c++20
//@ strict-diags
//@ diag: 30:14 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 31:27 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 32:13 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 33:24 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 34:25 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 35:27 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 36:28 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 37:24 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 38:29 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 39:14 warning "encoding prefix 'L' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 40:14 warning "encoding prefix 'u' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 41:14 warning "encoding prefix 'U' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 42:14 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 43:18 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 44:14 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 45:14 warning "encoding prefix 'L' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 45:19 warning "encoding prefix 'u' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 46:21 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
//@ diag: 47:8 warning "encoding prefix 'u8' on an unevaluated string literal has no effect and is incompatible with c++2c"
[[deprecated(u8"x")]] void f1();
__attribute__((deprecated(u8"x"))) void f2();
[[nodiscard(u8"x")]] int f3();
__attribute__((section(u8".text.x"))) void f4() {}
__attribute__((annotate(u8"x"))) void f5();
__attribute__((visibility(u8"hidden"))) void f6();
__attribute__((unavailable(u8"x"))) void f7();
__attribute__((abi_tag(u8"x"))) void f8();
__attribute__((enable_if(1, u8"x"))) void f9();
[[deprecated(L"x")]] void g1();
[[deprecated(u"x")]] void g2();
[[deprecated(U"x")]] void g3();
[[deprecated(u8R"(x)")]] void g4();
[[deprecated("a" u8"b")]] void g5();
[[deprecated(u8"a" "b")]] void g6();
[[deprecated(L"a" u"b")]] void g7();
static_assert(true, u8"x");
extern u8"C" void h1();
