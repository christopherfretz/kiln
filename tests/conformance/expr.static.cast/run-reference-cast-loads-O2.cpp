// [expr.static.cast]/4, [expr.reinterpret.cast]/11: the value of a cast to a
// reference type is a load of the cast's type from the operand's object
// ([conv.lval]), as run-reference-cast-of-reference-call-converted.cpp and
// expr.reinterpret.cast/run-reference-cast-loaded-as-target-type.cpp check
// at -O0; here optimized. The shapes of a downstream source file
// (`reinterpret_cast<const float&>(f.val_)` passed by value) and of boost
// optional's `assign(optional<U>&&)` (`static_cast<U&&>(rhs.get())`
// converted to another arithmetic type).
//@ expect: run
//@ args: -O2
//@ exit: 34
int show(float f) { return f == 2.5f ? 3 : 1; }
template <int W> struct OF {
  unsigned val_;
  friend int out(const OF &f) { return show(reinterpret_cast<const float &>(f.val_)); }
};
int plain(const unsigned &u) { return show(reinterpret_cast<const float &>(u)); }
template <class T> struct opt {
  T s;
  bool init;
  template <class U> void assign(opt<U> &&rhs) {
    if (rhs.init) s = static_cast<U &&>(rhs.get());
    init = rhs.init;
  }
  T &get() { return s; }
};
int main() {
  OF<1> a{0x40200000u};
  opt<unsigned> o{0, false};
  opt<unsigned short> p{4, true};
  o.assign(static_cast<opt<unsigned short> &&>(p));
  opt<double> d{0, false};
  opt<float> f{0.5f, true};
  d.assign(static_cast<opt<float> &&>(f));
  return out(a) * 10 + plain(0x40200000u) + o.s - (d.s == 0.5 ? 3 : 0);
}
