// [temp.inst] (WP-182): a function template specialization has its
// pattern's attributes (clang InstantiateAttrs), so a use of it is warned
// as for a non-template, naming the specialization with its arguments.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 16:3 warning "'depf<int>' is deprecated"
//@ diag: 12:21 note "'depf<int>' has been explicitly marked deprecated here"
//@ diag: 17:3 warning "'depn<double>' is deprecated: old"
//@ diag: 13:21 note "'depn<double>' has been explicitly marked deprecated here"
template<class T> [[nodiscard]] T ndf(T t) { return t; }
template<class T> [[deprecated]] void depf(T) {}
template<class T> [[deprecated("old")]] void depn(T) {}
void use() {
  ndf(1);
  depf(1);
  depn(1.0);
}
