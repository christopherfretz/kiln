// [temp.spec.partial.general]/9.5: the template parameter list of a partial
// specialization shall not contain default template arguments.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:21 error "default template argument in a class template partial specialization"
template <class T, class U> struct A {};
template <class T = int> struct A<T, char> {};
template <class T, class U> constexpr int v = 0;
template <class T = long> constexpr int v<T, char> = 1;
