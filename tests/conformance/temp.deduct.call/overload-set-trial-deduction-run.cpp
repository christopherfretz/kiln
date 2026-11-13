// [temp.deduct.call]/6.2: an argument that is an overload set with no
// function template is deduced against each member in turn; the type of
// the only member for which deduction succeeds is the argument (several
// succeeding: a non-deduced context, /6.2 last sentence). Each trial
// starts afresh ([temp.deduct.type]/2), with the explicitly specified
// template arguments already substituted into P. A non-static member
// named `&X::f` is tried as a pointer to member function
// ([expr.unary.op]/3.1), a static one as a pointer to function; a
// `noexcept` member deduces against a P without `noexcept`
// ([temp.deduct.call]/4.2). The set is formed in a template and resolved
// at instantiation too. A template-id set takes the specialization each
// template's explicit arguments name (clang `ResolveOverloadForDeduction`).
// Reduced from a downstream field-extraction header, whose
// extract_from/extract_to_buffer take the getters of an internal namespace
// handles, overloaded as `bool getX(T*) const noexcept` and
// `T getX() const noexcept`. Matches clang 21.
//@ expect: run
//@ stdout: 1 2 3 4 5 5 5 7 8 9 10 11 12
#include <cstdio>

template <typename P, typename S> int f(bool (S::*)(P *)) { return sizeof(P) == sizeof(long) ? 4 : 1; }
template <typename P> int g(bool (*)(P *)) { return sizeof(P) == sizeof(char) ? 9 : 2; }
template <typename P> int gr(bool (&)(P *)) { return 3; }
template <typename P> int h(P *p, bool (*fn)(P *)) { return fn(p) ? 8 : 0; }

template <typename P, typename T, typename S>
P extract_from(T const &o, bool (S::*ptr)(P *) const) {
  P v = P();
  (o.*ptr)(&v);
  return v;
}

struct H {
  bool get(int *p) { *p = 1; return true; }
  int get() { return 0; }
};
bool fr(int *) { return true; }
int fr() { return 0; }

// The static member is tried as `bool (*)(int*)`, the non-static one as
// `bool (M::*)(long*)`: g takes the first, f the second.
struct M {
  static bool s(int *) { return true; }
  bool s(long *) { return true; }
};

enum Side { Bid = 4, Ask = 5 };
struct Handle {
  bool getSide(Side *s) const noexcept { *s = Ask; return true; }
  Side getSide() const noexcept { return Bid; }
  bool getOrderID(long *v) const noexcept { *v = 7; return true; }
  long getOrderID() const noexcept { return 0; }
};
// Both `bool get(T*) const` deduce P alone; the explicit P picks one.
struct H2 {
  bool get(int *v) const { *v = 6; return true; }
  bool get(Side *v) const { *v = Ask; return true; }
  int get() const { return 0; }
};
template <typename T> long via(T const &t) { return extract_from(t, &T::getOrderID); }

// Both deduce P, so the argument is non-deduced; P comes from the first
// argument and the overload set is resolved for `bool (*)(int*)`.
bool two(int *) { return true; }
bool two(long *) { return false; }
// A template in the set: non-deduced too.
template <typename T> bool tmix(T *) { return false; }
bool tmix(int *) { return true; }
// A template-id: `tid<char>` names `bool(char*)` and `bool(char, char)`;
// only the first deduces.
template <typename T> bool tid(T *) { return true; }
template <typename T> bool tid(T, T) { return true; }

// In a class template's member, `&Y::get` names the current
// instantiation's members, resolved when `m` is instantiated.
template <class T> struct Y {
  bool get(T *) { return true; }
  int get() { return 0; }
  int m() { return f(&Y::get) + 11; }
};

int main() {
  Handle hd;
  H2 h2;
  int i = 0;
  std::printf("%d %d %d %d %d %d %d %ld %d %d %d %d %d\n",
              f(&H::get),                              // 1
              g(&fr),                                  // 2
              gr(fr),                                  // 3
              f(&M::s),                                // 4
              int(extract_from(hd, &Handle::getSide)), // 5
              int(extract_from<Side>(hd, &Handle::getSide)), // 5
              int(extract_from<Side>(h2, &H2::get)),  // 5
              via(hd),                                 // 7
              h(&i, &two),                             // 8
              g(&tid<char>),                           // 9
              g(&M::s) + 8,                            // 10
              h(&i, &tmix) + 3,                        // 11
              Y<int>().m());                           // 12
}
