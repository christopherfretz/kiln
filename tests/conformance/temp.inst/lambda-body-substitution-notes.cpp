// [temp.inst]/2, [expr.prim.lambda.closure]/2: a lambda-expression in a
// template is substituted with its enclosing specialization, and an error
// in its body carries "while substituting into a lambda expression here"
// (at the body's `{`) above the specialization's chain, one per nesting
// level, printed once per error depth like any context (clang
// `LambdaExpressionSubstitution`; spec/sema-templates.md §10.1).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 50:12 error "member reference base type 'const int' is not a structure or union"
//@ diag: 50:9 note "while substituting into a lambda expression here"
//@ diag: 49:7 note "while substituting into a lambda expression here"
//@ diag: 53:15 note "in instantiation of function template specialization 'nested<int>' requested here"
//@ diag: 55:51 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 55:42 note "while substituting into a lambda expression here"
//@ diag: 56:14 note "in instantiation of variable template specialization 'vt<int>' requested here"
//@ diag: 58:46 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 58:37 note "while substituting into a lambda expression here"
//@ diag: 59:17 note "in instantiation of default function argument expression for 'da<int>' required here"
//@ diag: 61:45 error "member reference base type 'const int' is not a structure or union"
//@ diag: 61:42 note "while substituting into a lambda expression here"
//@ diag: 62:15 note "in instantiation of function template specialization 'twice<int>' requested here"
//@ diag: 61:52 error "member reference base type 'const int' is not a structure or union"
//@ diag: 61:45 error "member reference base type 'const long' is not a structure or union"
//@ diag: 61:42 note "while substituting into a lambda expression here"
//@ diag: 63:15 note "in instantiation of function template specialization 'twice<long>' requested here"
//@ diag: 61:52 error "member reference base type 'const long' is not a structure or union"
//@ diag: 38:19 error "member reference base type 'int' is not a structure or union"
//@ diag: 38:16 note "while substituting into a lambda expression here"
//@ diag: 41:16 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 44:30 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 44:19 note "while substituting into a lambda expression here"
//@ diag: 46:25 note "in instantiation of member function 'S<int>::m' requested here"
//@ diag: 66:44 error "member reference base type 'const int' is not a structure or union"
//@ diag: 66:41 note "while substituting into a lambda expression here"
//@ diag: 68:25 note "in instantiation of function template specialization 'C<int>::mm<int>' requested here"

template <class T> void f(T t) {
  auto l = [&] { t.foo(); };
  l();
}
void use_f() { f(1); }

template <class T> struct S {
  void m(T) { [=] { typename T::type x; (void)x; }(); }
};
void use_s() { S<int>().m(1); }

template <class T> void nested(T t) {
  [=] {
    [=] { t.bar(); }();
  }();
}
template void nested<int>(int);

template <class T> constexpr int vt = [] { return T::v; }();
int use_vt = vt<int>;

template <class T> void da(int = [] { return T::w; }()) {}
void use_da() { da<int>(); }

template <class T> void twice(T t) { [=] { t.a(); t.b(); }(); }
template void twice<int>(int);
template void twice<long>(long);

template <class T> struct C {
  template <class U> void mm(U u) { [=] { u.q(); }(); }
};
void use_c() { C<int>().mm(1); }
