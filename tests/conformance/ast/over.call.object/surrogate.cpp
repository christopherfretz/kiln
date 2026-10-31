//@ expect: ast
//@ oracle: clang
// [over.call.object]/2: a call through a surrogate call function converts
// the object by its conversion function, then calls the result.
typedef int F(int);
struct A { operator F *(); };
struct C { int operator()(long); operator F *(); };
int f(A a, C c) { return a(1) + c(1L); }
