// [temp.deduct.general]/7: a trailing return type follows the parameters
// lexically, so a parameter type is substituted first and its hard error
// fires even though the return type would fail (WP-958).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 10:20 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 17:28 note "in instantiation of template class 'trap<char>' requested here"
//@ diag: 19:17 note "while substituting deduced template arguments into function template 'f' [with T = char]"
template <typename T> struct trap {
  typedef typename T::trap type;
};
template <bool, typename T = void> struct enable_if {
  typedef T type;
};
template <typename T> struct enable_if<false, T> {};
template <typename T>
auto f(const T &, typename trap<T>::type * = 0) -> typename enable_if<sizeof(T) == 17>::type;
void f(...);
void test_f() { f('a'); }
