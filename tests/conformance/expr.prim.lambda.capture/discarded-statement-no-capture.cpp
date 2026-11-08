// [stmt.if]/2, [basic.def.odr]/5: a name in a discarded statement is not
// odr-used, so a default capture does not capture it and the closure
// stays empty (clang treats the reference as FormallyOdrUsed).
//@ expect: run

int main() {
  int a = 1, b = 2;
  auto l = [=] {
    if constexpr (false) {
      return a + b;
    }
    return 0;
  };
  auto r = [&] {
    if constexpr (true) {
    } else {
      return a;
    }
    return 0;
  };
  static_assert(sizeof(l) == 1);
  static_assert(sizeof(r) == 1);
  return l() + r();
}
