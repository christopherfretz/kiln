// [dcl.init.list]/3.10, /3.9: a braced initializer binding a const
// reference member through a temporary still narrows. clang reports a
// narrowing in an aggregate's reference member under
// -Wc++11-narrowing-const-reference (an error by default), without the
// cast note unless the reference type is spelled through a typedef.
//@ verify

struct A { const int &r; };
struct B { const long &r; };
struct C { const char &r; };
struct D { int &&r; };
typedef const int &R;
struct E { R r; };
struct F { const volatile int &r; };
template <class T> struct G { const T &r; };
long l; int i; double d;
A a1{l}; // expected-error {{non-constant-expression cannot be narrowed from type 'long' to 'int' in initializer list}}
B b1{i};
C c1{300}; // expected-error {{constant expression evaluates to 300 which cannot be narrowed to type 'char'}} expected-warning {{implicit conversion from 'int' to 'const char' changes value from 300 to 44}}
C c2{65};
D d1{d}; // expected-error {{type 'double' cannot be narrowed to 'int' in initializer list}}
E e1{d}; // expected-error {{type 'double' cannot be narrowed to 'int' in initializer list}} expected-note {{insert an explicit cast to silence this issue}}
A a2{i};
G<int> g1{d}; // expected-error {{type 'double' cannot be narrowed to 'int' in initializer list}}
template <class T> auto h(T t) -> decltype(A{t}, 0);
template <class T> long h(...);
static_assert(sizeof(h<double>(0)) == sizeof(long));
