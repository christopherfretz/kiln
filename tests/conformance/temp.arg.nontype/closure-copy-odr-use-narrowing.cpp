// [temp.arg.nontype]/1, [dcl.init.list]/7: the narrowing check of a
// non-type template argument that odr-uses a by-copy capture reads the
// captured variable's value, as the argument's value does
// (closure-copy-odr-use-reads-variable-run.cpp): 300 does not fit
// `unsigned char`. clang 19 and 22 agree (GCC 16.1 does not diagnose it).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:39 error "non-type template argument evaluates to 300, which cannot be narrowed to type 'unsigned char'"
//@ diag: 12:31 warning "expression result unused"
template<unsigned char N> struct U {};
template<int N> struct T {};
void f() { const int i = 300; [=] { U<*&i> u; T<*&i> t; (void)u; (void)t; }; }
