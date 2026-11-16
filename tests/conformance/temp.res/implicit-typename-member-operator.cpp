// [temp.res.general]/4 (P0634, C++20): in a member-declaration a qualified
// name in the decl-specifier-seq is a type without `typename`, also before
// an operator-function-id (`A::ab operator[](int);`). Reduced from GCC
// PR123383; found running GCC's C++ testsuite (WP-1466).
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ xfail(kiln): WP-1495, implicit typename not applied before 'operator' in a member-declaration
template<class A> struct l { A::ab operator[](int); };
struct M { typedef int ab; };
l<M> x;
