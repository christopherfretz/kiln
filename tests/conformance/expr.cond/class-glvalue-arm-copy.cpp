// [expr.cond]/7: when the result is a class prvalue, a glvalue arm
// copy-initializes it, so its copy or move constructor runs (an xvalue
// arm is moved from). libc++ `optional::value_or() &&`.
//@ expect: run
struct X {
  int i_;
  constexpr X(int i) : i_(i) {}
  constexpr X(const X& x) : i_(x.i_ + 100) {}
  constexpr X(X&& x) : i_(x.i_) { x.i_ = 0; }
};
template <class T> constexpr T&& mv(T& t) { return static_cast<T&&>(t); }

constexpr int moved(bool b) {
  X x(2);
  X r = b ? mv(x) : X(3);
  return r.i_ * 10 + x.i_;
}
static_assert(moved(true) == 20);
static_assert(moved(false) == 32);

constexpr int copied(bool b) {
  X x(2);
  X r = b ? x : X(3);
  return r.i_ * 10 + x.i_;
}
static_assert(copied(true) == 1022);

// An lvalue and an xvalue of the same class: both arms are prvalues.
constexpr int mixed(bool b) {
  X x(2), y(5);
  X r = b ? x : mv(y);
  return r.i_ * 100 + x.i_ * 10 + y.i_;
}
static_assert(mixed(true) == 10225);
static_assert(mixed(false) == 520);

// The same glvalue category stays a glvalue: no copy.
constexpr int same(bool b) {
  X x(2), y(5);
  X& r = b ? x : y;
  r.i_ = 7;
  return x.i_ * 10 + y.i_;
}
static_assert(same(true) == 75);

int main() {
  bool t = moved(true) == 20 && copied(true) == 1022 && mixed(false) == 520;
  volatile bool b = true;
  X x(2);
  X r = b ? mv(x) : X(3);
  return t && r.i_ == 2 && x.i_ == 0 ? 0 : 1;
}
