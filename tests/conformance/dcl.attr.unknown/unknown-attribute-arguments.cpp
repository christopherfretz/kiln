// The argument clause of an attribute clang does not know
// ([dcl.attr.grammar]/5, clang `ParseCXX11AttributeArgs`,
// `ParseAttributeArgsCommon`). In `[[...]]` it is balanced tokens,
// skipped unchecked: a prefixed string, an undeclared name or stray
// tokens are only "unknown attribute ignored" (`clang::deprecated` and
// `clang::nodiscard` are not clang spellings). In `__attribute__` it is
// a list of expressions (a lone identifier is an identifier argument,
// a braced list is allowed): an invalid one is an error and drops the
// attribute without the warning, and a pack expansion is not supported.
// A KNOWN attribute's arguments keep their checks. clang 22 names a
// scoped attribute with its scope; clang 19 does not. Reduced from real
// code. spec/sema.md §4.5, WP-1836.
//@ verify
[[clang::deprecated(u8"x")]] void f1(); // expected-warning-re {{unknown attribute '{{(clang::)?}}deprecated' ignored}}
[[foo::bar(u8"x", undeclared + 1, ][)]] void f2(); // expected-warning-re {{unknown attribute '{{(foo::)?}}bar' ignored}} expected-error {{expected ']'}}
[[gnu::foo(u8"x", undeclared + 1)]] void f3(); // expected-warning-re {{unknown attribute '{{(gnu::)?}}foo' ignored}}
[[foo(u8"x", undeclared + 1)]] void f4(); // expected-warning {{unknown attribute 'foo' ignored}}
[[foo::bar(1 2 ;)]] void f5(); // expected-warning-re {{unknown attribute '{{(foo::)?}}bar' ignored}}
[[clang::nodiscard(u8"x")]] int f6(); // expected-warning-re {{unknown attribute '{{(clang::)?}}nodiscard' ignored}}
[[foo, bar(), baz(1, (2), [3], {4})]] void f7(); // expected-warning {{unknown attribute 'foo' ignored}} expected-warning {{unknown attribute 'bar' ignored}} expected-warning {{unknown attribute 'baz' ignored}}
__attribute__((foo(u8"x"))) void g1(); // expected-warning {{unknown attribute 'foo' ignored}}
__attribute__((foo(undeclared + 1))) void g2(); // expected-error {{use of undeclared identifier 'undeclared'}}
__attribute__((foo(undeclared))) void g3(); // expected-warning {{unknown attribute 'foo' ignored}}
__attribute__((foo(a, b))) void g4(); // expected-error {{use of undeclared identifier 'b'}}
__attribute__((foo({1, 2}))) void g5(); // expected-warning {{unknown attribute 'foo' ignored}}
__attribute__((foo("a" "b", L"w"))) void g6(); // expected-warning {{unknown attribute 'foo' ignored}}
__attribute__((foo(int))) void g7(); // expected-error {{expected '(' for function-style cast or type construction}}
__attribute__((foo(1,,2))) void g8(); // expected-error {{expected expression}}
__attribute__((foo(1 2))) void g12(); // expected-error 2 {{expected ')'}} expected-error {{expected unqualified-id}}
__attribute__((foo(this))) void g9(); // expected-error {{invalid use of 'this' outside of a non-static member function}}
template<class... T> __attribute__((foo(T{}...))) void g10(); // expected-error {{attribute 'foo' does not support argument pack expansion}}
template<class... T> __attribute__((foo(T{}))) void g11(); // expected-error {{expression contains unexpanded parameter pack 'T'}}
// Known attributes: the arguments are still unevaluated strings.
__attribute__((nodiscard(u8"x"))) int k1(); // expected-error {{an unevaluated string literal cannot have an encoding prefix}}
[[deprecated(u8"x")]] void k2(); // expected-error {{an unevaluated string literal cannot have an encoding prefix}}
[[gnu::deprecated(u8"x")]] void k3(); // expected-error {{an unevaluated string literal cannot have an encoding prefix}}
[[clang::warn_unused_result(u8"x")]] int k4(); // expected-error {{an unevaluated string literal cannot have an encoding prefix}}
