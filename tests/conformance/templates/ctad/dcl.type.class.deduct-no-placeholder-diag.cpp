// [dcl.type.class.deduct]/2: an alias-declaration, a trailing return type
// and a template type argument do not admit a deduced class type: clang
// asks for template arguments outright. A type-id template argument with a
// declarator operator does form one, which is not allowed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:11 error "use of class template 'B' requires template arguments"
//@ diag: 23:26 note "template is declared here"
//@ diag: 26:14 error "use of class template 'B' requires template arguments"
//@ diag: 23:26 note "template is declared here"
//@ diag: 27:4 error "use of class template 'B' requires template arguments"
//@ diag: 23:26 note "template is declared here"
//@ diag: 28:4 error "use of class template 'B' requires template arguments; argument deduction not allowed in template argument"
//@ diag: 23:26 note "template is declared here"
//@ diag: 29:10 error "use of class template 'B' requires template arguments; argument deduction not allowed in template argument"
//@ diag: 23:26 note "template is declared here"
//@ diag: 30:4 error "use of class template 'B' requires template arguments; argument deduction not allowed in function return type"
//@ diag: 23:26 note "template is declared here"
//@ diag: 31:4 error "use of class template 'B' requires template arguments; argument deduction not allowed in template argument"
//@ diag: 23:26 note "template is declared here"
//@ diag: 32:18 error "use of class template 'B' requires template arguments"
//@ diag: 23:26 note "template is declared here"
template<class T> struct B { B(T); };
template<class> struct TT {};
using A = B;
auto f2() -> B;
TT<B> tt;
TT<B *> tp;
TT<const B> tc;
TT<B()> tf;
TT<B[2]> ta;
auto l = []() -> B { return 1; };
