// [temp.spec.general]/5.2 (clang: explicit instantiation of an explicitly
// specialized class has no effect): an explicit instantiation after an
// explicit specialization of the same arguments.
//@ strict-diags
//@ diag: 9:17 warning "explicit instantiation of 'A<int>' that occurs after an explicit specialization has no effect"
//@ diag: 8:20 note "previous template specialization is here"
template <class T> struct A {};
template <> struct A<int> {};
template struct A<int>;
