// [temp.expl.spec]/1, [temp.pre]/1: an empty template head before a
// declaration whose name is not a template-id is extraneous (clang's
// `err_template_tag_noparams` / `err_template_variable_noparams`); the
// class or variable is declared as if the head were absent.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:1 error "extraneous 'template<>' in declaration of struct 'D2'"
//@ diag: 16:1 error "extraneous 'template<>' in declaration of variable 'v'"
//@ diag: 18:1 error "extraneous 'template<>' in declaration of struct 'Y'"
//@ diag: 20:12 error "extraneous 'template<>' in declaration of struct 'I'"
//@ diag: 22:1 error "extraneous 'template<>' in declaration of union 'U'"
//@ diag: 23:1 error "extraneous 'template<>' in declaration of class 'K'"
//@ diag: 24:1 error "extraneous 'template<>' in declaration of struct 'L'"
template<> struct D2 { int x; };
D2 d2;
template<> int v = 1;
int w = v;
template<> struct Y;
Y *py;
struct O { template<> struct I {}; };
O::I oi;
template<> union U { int a; };
template<> class K final {};
template<> struct L : D2 {};
