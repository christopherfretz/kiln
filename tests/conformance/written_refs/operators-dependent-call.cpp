// Dependent calls and subscripts (spec/ast.md §18.13.6, the `(` rule):
// a call whose callee is not a function name and which is type-dependent
// (the callee or an argument, [temp.dep.expr]/1) is a DEPENDENT entry at
// `(` with no found set: a function call operator is a member
// ([over.call.object]/1). Each specialization has a row: the
// `operator()` selected (`how=member`; a member template's
// specialization with `found=` the template), a surrogate call function's
// conversion function ([over.call.object]/2, `how=member`), or
// `(builtin)` when the callee is a pointer to function ([expr.call]/1).
// A call naming a function (`f(t)`, `t.f()`) is the name's entry, not an
// operator's. `t[i]` and `t[i, j]` ([over.sub]/1) work the same way at
// `[`. A template never instantiated and a discarded `if constexpr`
// branch ([stmt.if]/2) have the entry and no row. `t.f(x)` naming a data
// member of class type is not covered (§18.13.6).
//@ written-refs: operators-dependent-call.refs
struct F {
  int operator()(int) const;
};
struct Less {
  template <class A, class B> bool operator()(const A &, const B &) const;
};
template <class T = void> struct less {
  bool operator()(const T &a, const T &b) const { return a < b; }
};
template <> struct less<void> {
  template <class A, class B> bool operator()(const A &a, const B &b) const {
    return a < b;
  }
};
struct Fp {
  using Ptr = int (*)(int);
  operator Ptr() const;
};
struct M {
  int operator[](int) const;
  int operator[](int, int) const;
};
int plain(int);

template <class T> int call(T t) { return t(1); }
template <class T> bool lt(T a, T b) {
  Less l;
  return l(a, b);
}
template <class T> bool lt2(T a, T b) { return less<>{}(a, b); }
template <class T> int deref(T *p) { return (*p)(2); }
template <class T> int sub(T t) { return t[0]; }
template <class T> int multi(T m) { return m[1, 2]; }
template <class T> int named(T t) { return plain(t); }
template <class T> int lam(T t) {
  return [](auto x) { return x; }(t);
}
template <class T> int never(T t) { return t(3); }
template <class T> int discard(T t) {
  if constexpr (sizeof(T) > 1000)
    return t(4);
  else
    return 0;
}

int run() {
  int a[2] = {};
  int (*fp)(int) = plain;
  return call(F{}) + call(Fp{}) + call(plain) + lt(1, 2) + lt2(1, 2) +
         deref(&fp) + sub(M{}) + sub(a) + multi(M{}) + named(1) + lam(1) +
         discard(F{});
}
