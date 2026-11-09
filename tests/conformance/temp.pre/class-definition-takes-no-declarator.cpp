// [temp.pre]/5: in a template-declaration that defines a class, the
// init-declarator-list shall be empty. clang (ParseClassSpecifier with any
// template head: a class template, a partial or explicit specialization, an
// out-of-line member class, an explicit instantiation recovered as a
// specialization) demands the ';' after the class body and then parses the
// rest as a new declaration (a member class template too). Plain
// declarations, local classes in a
// function template and member classes of a class template may have
// declarators.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:30 error "expected ';' after struct"
//@ diag: 31:31 error "a type specifier is required for all declarations"
//@ diag: 33:30 error "expected ';' after struct"
//@ diag: 35:62 error "expected ';' after struct"
//@ diag: 35:63 error "a type specifier is required for all declarations"
//@ diag: 36:28 error "expected ';' after struct"
//@ diag: 36:29 error "a type specifier is required for all declarations"
//@ diag: 37:17 error "explicit template instantiation cannot have a definition; if this definition is meant to be an explicit specialization, add '<>' after the 'template' keyword"
//@ diag: 37:27 error "expected ';' after struct"
//@ diag: 37:28 error "a type specifier is required for all declarations"
//@ diag: 39:36 error "expected ';' after struct"
//@ diag: 39:37 error "a type specifier is required for all declarations"
//@ diag: 40:54 error "expected ';' after struct"
//@ diag: 40:55 error "a type specifier is required for all declarations"
//@ diag: 41:29 error "expected ';' after union"
//@ diag: 41:30 error "a type specifier is required for all declarations"
//@ diag: 42:74 error "expected ';' after struct"
//@ diag: 42:75 error "a type specifier is required for all declarations"
template<class T> struct A {} a;
template<class T> struct C { } ;
template<class T> struct D {}
int x;
template<class T> struct X; template<class T> struct X<T*> {} x1;
template<> struct X<int> {} x2;
template struct X<long> {} x3;
template<class T> struct O { struct I; template<class U> struct J; };
template<class T> struct O<T>::I {} oi;
template<class T> template<class U> struct O<T>::J {} oj;
template<class T> union U {} u;
template<class T> struct M { struct B {} b; template<class U> struct N {} n; };
template<class T> struct M2 { struct B {} b, *pb; enum E { e } ev; };
template<> struct X<char> {};
template<class T> struct X<T&> {};
template<class T> void f() { struct L {} l; (void)l; }
template<class T> auto g() { struct G { int v; } m{1}; return m; }
