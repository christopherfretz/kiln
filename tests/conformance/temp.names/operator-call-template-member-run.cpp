// [temp.names]/1, /3: inside a member function an unqualified
// `operator()<T>(...)` names the class's call-operator template, also
// when the overload set mixes it with a non-template `operator()` (only
// the templates are candidates for explicit template arguments,
// [temp.arg.explicit]) and through a lambda capturing `this`.
//@ expect: run
//@ std: c++26
//@ strict-diags
//@ stdout: 5 2 9 7
extern "C" int printf(const char*, ...);
struct T {
  template <class U> int operator()(U u) { return int(sizeof(U)) + u; }
  int g() { return operator()<char>(4); }
};
struct M {
  int operator()(int x) { return x; }
  template <class U> int operator()(U, U) { return 2; }
  int g() { return operator()<int>(1, 2); }
};
struct L {
  template <int N> int operator()(int x) { return N + x; }
  int g() { return [this] { return operator()<4>(5); }(); }
};
template <class X> struct D {
  template <class U> X operator()(U u) { return X(u) + 1; }
  X g() { return operator()<long>(6L); }
};
struct C {
  template <int N> constexpr int operator()() const { return N; }
  constexpr int g() const { return operator()<3>(); }
};
static_assert(C().g() == 3);
int main() { printf("%d %d %d %d\n", T().g(), M().g(), L().g(), D<int>().g()); }
