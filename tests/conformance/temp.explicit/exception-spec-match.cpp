// [temp.explicit]/9, [temp.deduct.decl], [except.spec]/5,13: an explicit instantiation is matched
// without the exception specifications; one it writes must then agree with the
// specialization's, instantiated if dependent; no specification does not match
// noexcept(false) here (clang `err_mismatched_exception_spec_explicit_instantiation`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 41:25 note "explicit instantiation refers here"
//@ diag: 43:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 41:25 note "explicit instantiation refers here"
//@ diag: 44:17 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 41:25 note "explicit instantiation refers here"
//@ diag: 49:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 48:25 note "explicit instantiation refers here"
//@ diag: 50:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 48:25 note "explicit instantiation refers here"
//@ diag: 51:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 48:25 note "explicit instantiation refers here"
//@ diag: 53:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 52:18 note "explicit instantiation refers here"
//@ diag: 55:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 41:25 note "explicit instantiation refers here"
//@ diag: 59:15 error "duplicate explicit instantiation of 'd<char>'"
//@ diag: 58:15 note "previous explicit instantiation is here"
//@ diag: 60:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 56:25 note "explicit instantiation refers here"
//@ diag: 67:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 65:23 note "explicit instantiation refers here"
//@ diag: 70:15 error "explicit instantiation of 'r' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 69:24 note "candidate template ignored: couldn't infer template argument 'B'"
//@ diag: 75:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 73:27 note "explicit instantiation refers here"
//@ diag: 77:15 error "explicit instantiation of 'f' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 41:25 note "candidate template ignored: could not match 'void (int) noexcept' against 'void (long)'"
//@ diag: 78:15 error "explicit instantiation of 'f' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 41:25 note "candidate template ignored: could not match 'void (int) noexcept' against 'void (long) noexcept'"
//@ diag: 79:39 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 80:10 note "in instantiation of exception specification for 'v<int>' requested here"
//@ diag: 80:10 error "exception specification in explicit instantiation does not match instantiated one"
//@ diag: 79:25 note "explicit instantiation refers here"
template <class T> void f(T) noexcept {}
template void f<int>(int) noexcept(false);
template void f(long) noexcept(false);
extern template void f<short>(short) noexcept(false);
template void f<char>(char) noexcept;
template void f<char *>(char *) throw();
template void f<bool>(bool);
template <class T> void g(T) {}
template void g<int>(int) noexcept;
template void g(long) noexcept(true);
template void g<char>(char) noexcept(false);
template <> void f<int *>(int *) noexcept;
template void f<int *>(int *) noexcept(false);
template void f<long *>(long *);
template void f<long *>(long *) noexcept(false);
template <class T> void d(T) noexcept(sizeof(T) > 1) {}
template void d<int>(int) noexcept;
template void d<char>(char);
template void d<char>(char) noexcept(false);
template void d<long>(long) noexcept(false);
template void d(short) noexcept(true);
template <class T> void e(T) noexcept(noexcept(T())) {}
template void e<int>(int) noexcept;
template void e<int *>(int *) noexcept(true);
template <int N> void q() noexcept(N > 0) {}
template void q<1>() noexcept;
template void q<0>() noexcept;
template void q<-1>();
template <bool B> void r() noexcept(B) {}
template void r() noexcept;
template void r<false>() noexcept(false);
template <class T> struct Y {
  template <class U> void m(U) noexcept {}
};
template void Y<int>::m<int>(int) noexcept(false);
template void Y<int>::m<long>(long) noexcept;
template void f<int>(long);
template void f<int>(long) noexcept;
template <class T> void v(T) noexcept(T::value) {}
template void v<int>(int) noexcept(false);
