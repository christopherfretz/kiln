// [temp.explicit]/3, /6: the type in an explicit instantiation of a
// variable template specialization must match the type the template gives it
// (clang's err_invalid_var_template_spec_type, at the type specifier, with
// a note at the template's latest declaration), for `template` and
// `extern template` alike; checked after the redeclaration rules
// ([temp.explicit]/8, /11, [temp.spec.general]/5), and a mismatched explicit
// instantiation still counts for a later one
//@ expect: compile-fail
//@ strict-diags
//@ diag: 51:17 error "type 'int' of explicit instantiation of 'vv' does not match expected type 'long'"
//@ diag: 38:22 note "variable template 'vv' declared here"
//@ diag: 52:10 error "type 'int' of explicit instantiation of 'vv' does not match expected type 'short'"
//@ diag: 38:22 note "variable template 'vv' declared here"
//@ diag: 54:22 error "explicit instantiation declaration (with 'extern') follows explicit instantiation definition (without 'extern')"
//@ diag: 51:21 note "explicit instantiation definition is here"
//@ diag: 56:10 error "type 'int' of explicit instantiation of 'cv' does not match expected type 'const int'"
//@ diag: 39:28 note "variable template 'cv' declared here"
//@ diag: 57:20 error "duplicate explicit instantiation of 'cv<int>'"
//@ diag: 56:14 note "previous explicit instantiation is here"
//@ diag: 58:10 error "type 'int *' of explicit instantiation of 'pv' does not match expected type 'long *'"
//@ diag: 40:23 note "variable template 'pv' declared here"
//@ diag: 59:16 error "type 'const int *' of explicit instantiation of 'pv' does not match expected type 'int *'"
//@ diag: 40:23 note "variable template 'pv' declared here"
//@ diag: 60:10 error "type 'long' of explicit instantiation of 'sm' does not match expected type 'int'"
//@ diag: 44:25 note "variable template 'sm' declared here"
//@ diag: 61:17 error "type 'unsigned int' of explicit instantiation of 'sm' does not match expected type 'unsigned long'"
//@ diag: 44:25 note "variable template 'sm' declared here"
//@ diag: 65:15 warning "explicit instantiation of 'vv<char>' that occurs after an explicit specialization has no effect"
//@ diag: 64:17 note "previous template specialization is here"
//@ diag: 65:10 error "type 'long' of explicit instantiation of 'vv' does not match expected type 'int'"
//@ diag: 38:22 note "variable template 'vv' declared here"
//@ diag: 67:17 error "type 'int' of explicit instantiation of 'vv' does not match expected type 'unsigned char'"
//@ diag: 38:22 note "variable template 'vv' declared here"
//@ diag: 68:17 error "type 'int' of explicit instantiation of 'vv' does not match expected type 'unsigned char'"
//@ diag: 38:22 note "variable template 'vv' declared here"
//@ diag: 71:17 error "duplicate explicit instantiation of 'vv<double>'"
//@ diag: 69:11 note "previous explicit instantiation is here"
template <class T> T vv = T();
template <class T> const T cv = T();
template <class T> T *pv = nullptr;
struct S {
  template <class T> static T sm;
};
template <class T> T S::sm = T();
template <class T> struct C {
  static T s;
};
template <class T> T C<T>::s = T();
template <class T> using Id = T;

extern template int vv<long>;
template int vv<short>;
template long vv<long>;
extern template long vv<long>;
template Id<int> vv<int>;
template int cv<int>;
template const int cv<int>;
template int *pv<long>;
template const int *pv<int>;
template long S::sm<int>;
extern template unsigned S::sm<unsigned long>;
// No type check for a static data member or an explicit specialization (as clang).
template int C<long>::s;
template <> int vv<char> = 5;
template long vv<char>;
extern template int vv<char>;
extern template int vv<unsigned char>;
extern template int vv<unsigned char>;
int use = vv<double>;
template double vv<double>;
template double vv<double>;
