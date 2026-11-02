// [temp.names]/3 (P0846): an unqualified name that ordinary lookup finds
// nothing for, or only (non-template) functions, followed by `<` is a
// template-name; argument-dependent lookup then finds the function
// template at the call, also at instantiation (libc++
// complex.tuple/get: `get<0>(c)` without `using std::get`).
//@ expect: run
//@ strict-diags
namespace N {
struct C { int a, b; };
template <int I> constexpr int& get(C& c) { return I == 0 ? c.a : c.b; }
template <class T> constexpr int put(T, C& c) { return c.b; }
}
int put(int);
template <class T> int f(T& c) { return get<1>(c); }
int main() {
  N::C c{3, 4};
  get<0>(c) += 2;
  int r = f(c) + put<int>(1, c);
  return c.a == 5 && r == 8 ? 0 : 1;
}
