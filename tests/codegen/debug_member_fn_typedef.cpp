// Debug info ([dcl.typedef]/1, [dcl.fct]/6, [class.mfct]): member
// functions declared through a function typedef, an alias and `decltype`. The
// declaration's and the definition's `type:` are DISubroutineTypes with
// the object pointer (`const`-qualified for `typedef int CF() const;`),
// the reference qualifier as DIFlagRValueReference, `noreturn` as
// DIFlagNoReturn, and a virtual member's vtable index, as clang writes
// them (clang `getOrCreateMethodType` sees through the top-level sugar).
// Reduced from real code.
//@ args: -g
typedef int CF() const;
typedef int RF() &&;
typedef void NF() __attribute__((noreturn));
typedef int VF(int);
using AF = long(long) const;
struct S {
  int k;
  CF c;
  RF r;
  static NF n;
  virtual VF v;
  AF a;
  decltype(&S::v) p;
};
int S::c() const { return k; }
int S::r() && { return k; }
void S::n() { throw 1; }
int S::v(int x) { return x + k; }
long S::a(long x) const { return x; }

int run(S s) {
  return s.c() + static_cast<S &&>(s).r() + s.v(1) + int(s.a(2));
}
