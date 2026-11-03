// [class.temporary]/3, [expr.call]/7 with clang's `[[clang::trivial_abi]]`
// (WP-460): a class with the attribute in effect, and a class that is
// trivial for calls only through its member, is passed and returned in
// registers; the callee destroys a by-value parameter at its end
// (`isParamDestroyedInCallee`), and the caller keeps only an EH cleanup
// for the argument, deactivated just before the call.
struct [[clang::trivial_abi]] P {
  long v;
  P(const P &);
  P(P &&);
  ~P();
};
struct W { P p; };
struct Q { Q(); ~Q(); };
int may();
void take(P, int);
void take_w(W);
void callee(P p) { may(); }
void callee_two(P a, W b) {}
void caller(P &a) { take(a, may()); }
void caller_temp(P &a) {
  Q q;
  take(static_cast<P &&>(a), 1);
}
void caller_w(W &w) { take_w(w); }
void nested(P &a) { take(P(a), (take(a, 2), 3)); }
void conditional(bool b, P &x, P &y) { take(b ? P(x) : P(y), may()); }
P make();
P make_local() {
  P r = make();
  may();
  return r;
}
void forward() {
  auto l = [](P p) { may(); };
  void (*fp)(P) = l;
  fp(make());
}
