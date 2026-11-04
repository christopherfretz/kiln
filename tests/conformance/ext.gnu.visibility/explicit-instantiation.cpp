// The visibility attribute of an explicit instantiation ([temp.explicit]
// GNU extension; spec/sema.md "Visibility attributes", clang
// ActOnExplicitInstantiation / mergeVisibilityAttr): it is checked like
// any other, and one that differs from an earlier explicit
// instantiation's is an error at the earlier attribute.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:32 error "visibility does not match previous declaration"
//@ diag: 13:32 note "previous attribute is here"
//@ diag: 14:36 warning "'visibility' attribute argument not supported: bogus"
template <class T> __attribute__((visibility("hidden"))) T f(T t) { return t; }
extern template __attribute__((visibility("default"))) int f<int>(int);
extern template __attribute__((visibility("protected"))) int f<int>(int);
template __attribute__((visibility("bogus"))) short f<short>(short);
