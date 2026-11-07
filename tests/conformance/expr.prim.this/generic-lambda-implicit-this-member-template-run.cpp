// [expr.prim.this]/2, [expr.prim.id.general]/3, [class.mfct.non.static]/2:
// in the body of a generic lambda, `this` is the `this` of the enclosing
// member function (or of the class whose default member initializer holds
// the lambda), also when the call operator specialization is instantiated
// after the lambda. So `Get<decltype(v)>(0)`, a member template named
// with dependent template arguments, is the implicit `this->Get<...>(0)`
// at instantiation, through `[&]`, `[this]`, `[*this]` and nested lambdas;
// the lambda captures `this` at its definition ([expr.prim.lambda.capture]/8),
// also when a static member template is a candidate too, unless it has
// no capture-default (then only the static one may be called).
//@ expect: run
//@ strict-diags
//@ std: c++20

struct M {
  int base = 10;
  template <typename T> T Get(int k) const { return T(base + k); }
  template <typename T> T Set(T v) { base = int(v); return v; }
  template <typename T> T Mix(int k) const { return T(base * k); }
  template <typename T> static T Mix(long k) { return T(k); }

  int by_ref() {
    auto l = [&](auto v) { return Get<decltype(v)>(1) + v; };
    return l(2);
  }
  int by_this() {
    auto l = [this](auto v) {
      Set<decltype(v)>(v);
      return Get<decltype(v)>(0);
    };
    return l(5);
  }
  int by_copy() const {
    auto l = [*this](auto v) { return Get<decltype(v)>(v); };
    return l(3);
  }
  int nested() {
    auto outer = [&](auto a) {
      auto inner = [&](auto b) { return Get<decltype(a + b)>(int(a + b)); };
      return inner(a);
    };
    return outer(4);
  }
  int mixed() const {
    auto l = [&](auto v) { return Mix<decltype(v)>(3) + Mix<decltype(v)>(4L); };
    auto s = [](auto v) { return Mix<decltype(v)>(long(v)); };
    return l(1) + s(1);
  }
  int field = [this](auto v) { return Get<decltype(v)>(v); }(7);
};

template <typename U> struct N {
  U u = 2;
  template <typename T> T twice() const { return T(u * 2); }
  U run() const {
    auto l = [&](auto v) { return twice<decltype(v)>() + v; };
    return l(U(1));
  }
};

int main() {
  M m;
  if (m.field != 17) return 1;
  if (m.by_ref() != 13) return 2;
  if (m.by_this() != 5) return 3;
  if (m.by_copy() != 8) return 4;
  if (m.nested() != 13) return 5;
  if (N<int>{}.run() != 5) return 6;
  if (m.mixed() != 20) return 7;
  return 0;
}
