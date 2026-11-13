// [dcl.attr.deprecated]/4, [expr.prim.lambda.general]/4 (C++23): an
// attribute-specifier-seq after the lambda-introducer appertains to the
// function call operator (or operator template), so calling a
// `[[deprecated]]` lambda warns; so does calling any deprecated overloaded
// operator (clang `CreateFunctionRefExpr`). For a member operator template
// (a generic lambda) clang 19 and 22 warn about both the template and the
// specialization.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
//@ diag: 30:4 warning "'operator()' is deprecated"
//@ diag: 29:17 note "'operator()' has been explicitly marked deprecated here"
//@ diag: 32:4 warning "'operator()' is deprecated: use n"
//@ diag: 31:17 note "'operator()' has been explicitly marked deprecated here"
//@ diag: 34:4 warning "'operator()' is deprecated"
//@ diag: 33:12 note "'operator()' has been explicitly marked deprecated here"
//@ diag: 34:4 warning "'operator()<int>' is deprecated"
//@ diag: 33:17 note "'operator()<int>' has been explicitly marked deprecated here"
//@ diag: 36:4 warning "'operator()' is deprecated"
//@ diag: 35:17 note "'operator()' has been explicitly marked deprecated here"
//@ diag: 40:3 warning "'operator()' is deprecated"
//@ diag: 42:15 note "in instantiation of function template specialization 't<int>' requested here"
//@ diag: 39:17 note "'operator()' has been explicitly marked deprecated here"
//@ diag: 47:21 warning "'operator()' is deprecated"
//@ diag: 44:5 note "'operator()' has been explicitly marked deprecated here"
//@ diag: 47:27 warning "'operator+' is deprecated"
//@ diag: 45:5 note "'operator+' has been explicitly marked deprecated here"
void use() {
  auto l = [] [[deprecated]] () {};
  l();
  auto m = [] [[deprecated("use n")]] (int) {};
  m(1);
  auto g = [] [[deprecated]] (auto) {};
  g(1);
  auto p = [] [[deprecated]] {};
  p();
}
template <class T> void t() {
  auto l = [] [[deprecated]] (T) {};
  l(T());
}
template void t<int>();
struct S {
  [[deprecated]] void operator()();
  [[deprecated]] S operator+(S);
};
void s(S a, S b) { a(); a + b; }
