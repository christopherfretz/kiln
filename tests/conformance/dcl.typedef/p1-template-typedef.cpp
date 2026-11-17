// [temp.pre]/2: a template-declaration does not declare a typedef: an alias
// template is spelled with using.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:31 error "a typedef cannot be a template"
//@ diag: 11:15 warning "typedef requires a name [-Wmissing-declarations]"
//@ diag: 12:29 error "a typedef cannot be a template"
//@ diag: 13:40 error "a typedef cannot be a template"
template<class T> typedef int X;
template<int> typedef struct A;
template<class T> typedef T Y;
struct M { template<class T> typedef T Z; };
template<class T> using OK = T;
OK<int> ok = 0;
