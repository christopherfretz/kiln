// [dcl.init.list]/3.7, [over.match.list]: when a braced list initializes
// a class through a constructor, a narrowing conversion of an argument is
// ill-formed; a reference parameter bound to a temporary of arithmetic
// type narrows like the temporary's initialization ([dcl.init.ref]/5.4).
// clang reports it under -Wc++11-narrowing-const-reference.
//@ verify

struct S { int a, b; };
struct P { P(const signed char &, const S &); };
struct Q { Q(const signed char &, int); };
struct R { R(signed char &&, const int &); };
struct V { V(const volatile signed char &); };
struct W { W(const int &, const long &); };
template <class T1, class T2> struct Pair { Pair(const T1 &, const T2 &); };
unsigned char u();
signed char sc();
int &ri();
long l;
void f(P);
void f2(Q);
void f3(Pair<const signed char, S>);

void g() {
  f({u(), {1, 2}}); // expected-error {{non-constant-expression cannot be narrowed from type 'unsigned char' to 'signed char' in initializer list}}
  f2({u(), 1}); // expected-error {{non-constant-expression cannot be narrowed from type 'unsigned char' to 'signed char' in initializer list}}
  P p{u(), {1, 2}}; // expected-error {{non-constant-expression cannot be narrowed from type 'unsigned char' to 'signed char' in initializer list}}
  R r{u(), 1}; // expected-error {{non-constant-expression cannot be narrowed from type 'unsigned char' to 'signed char' in initializer list}}
  R r2{sc(), l}; // expected-error {{non-constant-expression cannot be narrowed from type 'long' to 'int' in initializer list}}
  W w{ri(), ri()};
  W w2{l, 1}; // expected-error {{non-constant-expression cannot be narrowed from type 'long' to 'int' in initializer list}}
  Q q{static_cast<signed char>(u()), 1};
  Q q2{'a', 1};
  Q q3(u(), 1);
  f3({u(), {1, 2}}); // expected-error {{cannot be narrowed from type 'unsigned char' to}}
  f3({sc(), {1, 2}});
}

// A volatile reference does not bind to a temporary.
void h(V);
void h2() { signed char c = 0; V v{c}; (void)v; }
