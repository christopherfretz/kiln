// [temp.pre]/2 valid twin: an enumeration type named by an
// elaborated-type-specifier in a variable or function template, and a
// member enumeration of a class template defined out of line, are fine
// (WP-1063).
//@ expect: compile-ok
enum E { a, b };
template<class T> enum E v = a;
template<class T> enum E f(T) { return b; }
template<class T> struct A { enum class M : int; };
template<class T> enum class A<T>::M : int { x, y };
int use() { return v<int> + f(0) + static_cast<int>(A<char>::M::y); }
