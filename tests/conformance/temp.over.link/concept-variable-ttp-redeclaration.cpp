// [temp.over.link]/6.1 (P2841R7): template parameters are equivalent only
// if they are of the same kind, and a type, a concept and a variable
// template template parameter are three kinds. Redeclarations that rename
// a concept or variable template template parameter declare the same
// template; function templates whose heads differ only in that kind are
// distinct overloads, and an argument of the wrong kind removes a
// candidate (spec/sema-templates.md §6.8). clang 22 agrees. WP-746.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
template <class T> concept Int = __is_same(T, int);
template <class T> constexpr int sz = sizeof(T) == 1 ? 1 : 9;
template <class T> struct X { static constexpr int v = 7; };

template <template <class> auto V> constexpr int g();
template <template <class> auto W> constexpr int g() { return W<char>; }
static_assert(g<sz>() == 1);

template <template <class> concept C> struct S;
template <template <class> concept K> struct S { static constexpr bool b = K<int>; };
static_assert(S<Int>::b);

template <template <class> class K> constexpr int h() { return K<int>::v; }
template <template <class> concept K> constexpr int h() { return 2; }
template <template <class> auto K> constexpr int h() { return K<char> + 10; }
static_assert(h<X>() == 7 && h<Int>() == 2 && h<sz>() == 11);
