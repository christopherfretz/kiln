// [dcl.struct.bind], [dcl.attr.unused] (WP-217): -Wunused-variable for a
// structured binding declaration (clang ShouldDiagnoseUnusedDecl on a
// DecompositionDecl): diagnosed at its `[`, named `[a, b]`, when none of
// its bindings is referenced or marked unused (and not all are
// placeholders), whatever the declaration's storage or reference kind; one
// of dependent type is diagnosed as it is instantiated, with the
// instantiation note.
//@ args: -Wunused-variable
//@ strict-diags
//@ diag: 32:13 warning "unused variable '[j, k]'"
//@ diag: 27:8 warning "unused variable '[a, b]'"
//@ diag: 30:15 warning "unused variable '[h, i]'"
//@ diag: 34:9 warning "unused variable '[l, m]'"
//@ diag: 35:15 warning "unused variable '[n, o]'"
//@ diag: 44:8 warning "unused variable '[e, f]'"
//@ diag: 41:8 warning "unused variable '[a, b]'"
//@ diag: 46:15 note "in instantiation of function template specialization 'tf<P>' requested here"
struct P {
  int a, b;
};
struct NT {
  int a, b;
  ~NT();
};

void f() {
  auto [a, b] = P{1, 2};
  auto [c, d] = P{1, 2};
  (void)c;
  static auto [h, i] = P{3, 4};
  P arr[2] = {};
  for (auto [j, k] : arr) {
  }
  auto &[l, m] = arr[0];
  const auto &[n, o] = arr[1];
  auto [x, y] = NT{};
  [[maybe_unused]] auto [mu, mv] = P{};
}

template <class T> void tf(T t) {
  auto [a, b] = t;
  auto [c, d] = t;
  (void)d;
  auto [e, f] = P{};
}
template void tf<P>(P);
