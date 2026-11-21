// The argument clause of an attribute clang does not know, in C23
// (6.7.13.2: balanced tokens) and in `__attribute__` (expressions; an
// invalid one is an error that drops the attribute without the
// warning; only a first argument can be a lone identifier). `clang::warn_unused_result` is a C++-only spelling, so it
// is unknown here. clang 22 names a scoped attribute with its scope;
// clang 19 does not. spec/sema.md §4.5, WP-1836.
//@ std: c23
//@ verify
[[clang::deprecated(u8"x")]] void f1(void); // expected-warning-re {{unknown attribute '{{(clang::)?}}deprecated' ignored}}
[[gnu::foo(u8"x", undeclared + 1)]] void f2(void); // expected-warning-re {{unknown attribute '{{(gnu::)?}}foo' ignored}}
[[foo(L"x", 1 2 ;)]] void f3(void); // expected-warning {{unknown attribute 'foo' ignored}}
[[clang::warn_unused_result(u8"x")]] int f4(void); // expected-warning-re {{unknown attribute '{{(clang::)?}}warn_unused_result' ignored}}
__attribute__((foo(y, u8"x"))) void g1(void); // expected-warning {{unknown attribute 'foo' ignored}}
__attribute__((foo(undeclared))) void g2(void); // expected-warning {{unknown attribute 'foo' ignored}}
__attribute__((foo(undeclared + 1))) void g3(void); // expected-error {{use of undeclared identifier 'undeclared'}}
__attribute__((foo((struct { int x; }){1}))) void g4(void); // expected-warning {{unknown attribute 'foo' ignored}}
__attribute__((foo(1,,2))) void g5(void); // expected-error {{expected expression}}
// A known attribute's arguments keep their checks.
[[deprecated(u8"x")]] void k1(void); // expected-warning {{encoding prefix 'u8' on an unevaluated string literal has no effect}}
void use(void) { f4(); }
