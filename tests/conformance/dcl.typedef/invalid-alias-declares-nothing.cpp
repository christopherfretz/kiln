// [dcl.typedef]/2, [dcl.enum]/2: an alias-declaration whose type-id is
// invalid declares nothing (clang `ActOnAliasDeclaration` returns no
// declaration), so each later use of its name is an unknown type name;
// a typedef with an invalid type is still declared (as `int`). An
// enum-base whose type is invalid gives the enumeration the underlying
// type `int` (clang `ActOnTag`), so a constant expression may build and
// convert its values (WP-2173: kiln kept the alias, and the constant
// evaluator aborted converting `int` to the enumeration of its error type).
//@ verify
//@ args: -ferror-limit=0
using A = undeclared_a; // expected-error {{unknown type name 'undeclared_a'}}
A a; // expected-error {{unknown type name 'A'}}

using P = undeclared_p *; // expected-error {{unknown type name 'undeclared_p'}}
P p; // expected-error {{unknown type name 'P'}}

template <class T> using T1 = undeclared_t; // expected-error {{unknown type name 'undeclared_t'}}
T1<int> *t1; // expected-error {{no template named 'T1'}}

struct S {
  using M = undeclared_m; // expected-error {{unknown type name 'undeclared_m'}}
  M m; // expected-error {{unknown type name 'M'}}
};

typedef undeclared_td td; // expected-error {{unknown type name 'undeclared_td'}}
static_assert(sizeof(td) == sizeof(int));

using size_type = undeclared_size; // expected-error {{unknown type name 'undeclared_size'}}
enum class E : size_type {}; // expected-error {{unknown type name 'size_type'}}
static_assert(sizeof(E) == sizeof(int));
constexpr int e() { return static_cast<int>(E{7}); }
static_assert(e() == 7);

enum class F : decltype(undeclared_v) {}; // expected-error {{use of undeclared identifier 'undeclared_v'}}
constexpr int f() { return static_cast<int>(F{2}); }
static_assert(f() == 2);

enum U : td { u1 = 1 };
static_assert(sizeof(U) == sizeof(int) && u1 == 1);
