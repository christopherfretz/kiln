// [temp.expl.spec], [temp.deduct.decl], [except.spec]/5, [temp.friend]: an explicit
// specialization (or a friend naming a specialization) is matched without the exception
// specifications and then redeclares the specialization, whose specification is the
// instantiated one (clang `MergeFunctionDecl` -> `CheckEquivalentExceptionSpec`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:18 error "'h<int>' is missing exception specification 'noexcept'"
//@ diag: 33:18 note "previous declaration is here"
//@ diag: 34:18 error "exception specification in declaration does not match previous declaration"
//@ diag: 34:18 note "previous declaration is here"
//@ diag: 39:18 error "explicit specialization of 'h<short>' after instantiation"
//@ diag: 38:14 note "implicit instantiation first required here"
//@ diag: 41:18 error "exception specification in declaration does not match previous declaration"
//@ diag: 41:18 note "previous declaration is here"
//@ diag: 43:18 error "exception specification in declaration does not match previous declaration"
//@ diag: 43:18 note "previous declaration is here"
//@ diag: 46:18 error "exception specification in declaration does not match previous declaration"
//@ diag: 46:18 note "previous declaration is here"
//@ diag: 48:18 error "'d<long>' is missing exception specification 'noexcept(sizeof(long) > 1)'"
//@ diag: 48:18 note "previous declaration is here"
//@ diag: 51:18 error "exception specification in declaration does not match previous declaration"
//@ diag: 51:18 note "previous declaration is here"
//@ diag: 53:18 error "exception specification in declaration does not match previous declaration"
//@ diag: 53:18 note "previous declaration is here"
//@ diag: 56:18 error "no function template matches function template specialization 'r'"
//@ diag: 55:24 note "candidate template ignored: couldn't infer template argument 'B'"
//@ diag: 60:39 error "'m<int>' is missing exception specification 'noexcept'"
//@ diag: 60:39 note "previous declaration is here"
//@ diag: 63:15 error "'h<double>' is missing exception specification 'noexcept'"
//@ diag: 66:11 note "in instantiation of template class 'X<double>' requested here"
//@ diag: 63:15 note "previous declaration is here"
template <class T> void h(T) noexcept {}
template <> void h<int>(int);
template <> void h(long) noexcept(false);
template <> void h(char) noexcept;
template <> void h<char *>(char *) throw();
template <> void h<bool>(bool) noexcept {}
void use() { h(short()); }
template <> void h<short>(short) noexcept(false);
template <class T> void k(T) {}
template <> void k<int>(int) noexcept;
template <> void k<long>(long) noexcept(false);
template <> void k<char>(char) noexcept {}
template <class T> void d(T) noexcept(sizeof(T) > 1) {}
template <> void d<short>(short) noexcept;
template <> void d<bool>(bool) noexcept;
template <> void d<char>(char);
template <> void d<long>(long);
template <class T> void b(T) noexcept(noexcept(T())) {}
template <> void b<int>(int) noexcept {}
template <> void b<long>(long) noexcept(false) {}
template <int N> void q() noexcept(N > 0) {}
template <> void q<2>() noexcept(false);
template <> void q<-1>();
template <bool B> void r() noexcept(B) {}
template <> void r() noexcept(false);
template <class T> struct Y {
  template <class U> void m(U) noexcept {}
};
template <> template <> void Y<long>::m<int>(int);
template <> template <> void Y<long>::m<long>(long) noexcept;
template <class T> struct X {
  friend void h<>(T);
  friend void h<>(int *) noexcept;
};
X<double> xd;
struct Z {
  friend void k<>(unsigned *);
};
