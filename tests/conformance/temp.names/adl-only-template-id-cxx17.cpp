// [temp.names]/3 (P0846, C++20): an unqualified name followed by `<` for
// which ordinary lookup finds nothing is assumed to be a template-name,
// and the call finds its templates by argument-dependent lookup. Before
// C++20 clang accepts this as an extension (`ext_adl_only_template_id`,
// -Wc++20-extensions, on by default) at the name, after the call's own
// diagnostics: once for a non-dependent call in a template, and at the
// definition and again per instantiation for a dependent one. A call
// where ADL finds no template is an undeclared identifier, no warning.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 29:9 warning "use of function template name with no prior declaration in function call with explicit template arguments is a C++20 extension"
//@ diag: 30:9 warning "use of function template name with no prior declaration in function call with explicit template arguments is a C++20 extension"
//@ diag: 31:9 error "no matching function for call to 'q'"
//@ diag: 26:25 note "candidate function template not viable: no known conversion from 'M::S' to 'int' for 2nd argument"
//@ diag: 25:25 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 31:9 warning "use of function template name with no prior declaration in function call with explicit template arguments is a C++20 extension"
//@ diag: 32:9 error "use of undeclared identifier 'r'"
//@ diag: 33:39 warning "use of function template name with no prior declaration in function call with explicit template arguments is a C++20 extension"
//@ diag: 35:37 warning "use of function template name with no prior declaration in function call with explicit template arguments is a C++20 extension"
//@ diag: 33:39 warning "use of function template name with no prior declaration in function call with explicit template arguments is a C++20 extension"
//@ diag: 34:9 note "in instantiation of function template specialization 'h<M::S>' requested here"
namespace M {
  struct S {};
  template<class T> int q(S);
  template<class T> int q(S, int);
  int r(S);
}
int a = q<int>(M::S{});
int b = q<long>(M::S{}, 1);
int c = q<int>(M::S{}, M::S{});
int d = r<int>(M::S{});
template<class U> int h(U u) { return q<int>(u); }
int e = h(M::S{});
template<class U> int h2() { return q<int>(M::S{}); }
int f = h2<int>();
