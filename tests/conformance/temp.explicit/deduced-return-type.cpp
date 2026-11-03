// [temp.explicit]/7, [temp.deduct.decl], [dcl.spec.auto.general]/13: an
// explicit instantiation or specialization of a function template with a
// deduced return type names it with the same placeholder (the return type
// is a non-deduced context, clang `SubstAutoTypeDependent`), so a
// different or deduced type does not match; taking the address deduces
// the specialization's return type, which must then be the target's.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:21 error "explicit instantiation of 'fd' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 15:25 note "candidate template ignored: could not match 'auto (short)' against 'int (short)'"
//@ diag: 22:15 error "explicit instantiation of 'fe' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 16:35 note "candidate template ignored: could not match 'decltype(auto) (short)' against 'auto (short)'"
//@ diag: 25:19 error "address of overloaded function 'fd' does not match required type 'int (long)'"
//@ diag: 15:25 note "candidate template ignored: could not match 'long (long)' against 'int (long)'"
template <class T> auto fd(T t) { return t + 1; }
template <class T> decltype(auto) fe(T t) { return t; }
extern template auto fd(int);
template auto fd(long);
extern template decltype(auto) fe(int);
template <> auto fd(char c) { return c; }
extern template int fd(short);
template auto fe(short);
int (*p)(int) = fd;
auto (*r)(unsigned) = fd<unsigned>;
int (*q)(long) = &fd<long>;
int use() { return fd(1) + fe(2) + p(3) + r(4u); }
