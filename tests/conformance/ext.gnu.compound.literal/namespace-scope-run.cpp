// Namespace-scope compound literals ([GNU]) that are constant
// initializers: scalar, aggregate, vector and constexpr-constructor
// literals are prvalues as in a function and initialize their
// variables statically (clang emits the same constant globals).
//@ expect: run
//@ exit: 0
struct P { int a, b; };
struct C { constexpr C(int i) : i(i) {} int i; };
typedef int v4 __attribute__((vector_size(16)));
int x = (int){5};
P p = (P){1, 2};
constexpr int cx = (int){7};
v4 v = (v4){1, 2, 3, 4};
C c = (C){4};
template<class T> T vt = (T){9};
static_assert((int){3} == 3);
auto sz = sizeof((P){1, 2});
int main() { return x + p.b + cx + v[1] + c.i + vt<int> + (int)sz - 37; }
