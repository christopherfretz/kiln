// [class.temporary]/3, Itanium C++ ABI 3.1.2.3 (clang 19
// `canPassInRegisters`): a class with a non-trivial, non-deleted copy or
// move constructor or destructor is passed by address and returned
// through `sret`, even when its copy constructor is trivial. A deleted
// copy constructor does not count; a trivial move constructor alone
// keeps the class in registers.
struct M { int a, b; M() = default; M(const M &) = default; M(M &&) {} };
struct D { int a; D() = default; D(const D &) = delete; D(D &&) = default; };
struct DM { int a; DM() = default; DM(const DM &) = delete; DM(DM &&) {} };
struct DD { int a; DD() = default; DD(const DD &) = default; DD(DD &&) = delete; };
M take_m(M m) { return m; }
D take_d(D d) { return static_cast<D &&>(d); }
int use_dm(DM d) { return d.a; }
DM make_dm() { return DM(); }
int use_dd(DD d) { return d.a; }
int call() {
  M m;
  take_m(m);
  D d;
  take_d(static_cast<D &&>(d));
  DM x;
  DD y;
  return use_dm(static_cast<DM &&>(x)) + use_dm(make_dm()) + use_dd(y);
}
// Ignored results of member and operator calls taking a by-value class.
struct H {
  M get(M m) { return m; }
  virtual D vget(D d) { return static_cast<D &&>(d); }
  M operator+(M m) { return m; }
};
void ignored(H &h, M m) {
  h.get(m);
  h.vget(D());
  h + m;
  (take_m(m));
}
