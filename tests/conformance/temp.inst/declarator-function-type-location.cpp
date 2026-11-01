// [dcl.fct]/11, [temp.inst]/3: a member's parameter, data member, typedef,
// alias or variable whose declarator contains a function type that returns
// an array after substitution is ill-formed, diagnosed where that function
// type begins (clang's FunctionTypeLoc begin: the first type-specifier, `T`
// of `T (*p)()` and of `static const T (*s)()`), not at the declarator name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:17 error "function cannot return array type 'char[2]'"
//@ diag: 31:12 note "in instantiation of template class 'X<char[2]>' requested here"
//@ diag: 23:17 error "function cannot return array type 'char[2]'"
//@ diag: 24:10 error "function cannot return array type 'char[2]'"
//@ diag: 25:16 error "function cannot return array type 'const char[2]'"
//@ diag: 26:3 error "function cannot return array type 'char[2]'"
//@ diag: 27:16 error "function cannot return array type 'const char[2]'"
//@ diag: 28:11 error "function cannot return array type 'char[2]'"
//@ diag: 29:13 error "function cannot return array type 'char[2]'"
//@ diag: 32:43 error "function cannot return array type 'char[2]'"
//@ diag: 33:12 note "in instantiation of template class 'Y<char[2]>' requested here"
//@ diag: 34:38 error "function cannot return array type 'char[2]'"
//@ diag: 35:12 note "in instantiation of function template specialization 'v<char[2]>' requested here"
template <class T> struct X {
  void m(int a, T (*p)());
  void n(int a, T (*)(int), T (&r)());
  void o(T (*(*q)())());
  void c(const T (*p)());
  T (*fp)();
  static const T (*s)();
  typedef T (*P)();
  using U = T (*)();
};
X<char[2]> x;
template <class T> struct Y { void m(int, T f()); };
Y<char[2]> y;
template <class T> void v() { int a; T (*l)() = nullptr; }
void w() { v<char[2]>(); }
