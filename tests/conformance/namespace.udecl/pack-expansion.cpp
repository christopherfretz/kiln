// [namespace.udecl]/1 (P0195): a using-declarator followed by `...` is a
// pack expansion; each base's operator() joins the overload set
//@ expect: run
//@ strict-diags
template <class T, int I> struct Ov { int operator()(T) const { return I; } };
template <class... Bs> struct All : Bs... { using Bs::operator()...; };
int main() {
  All<Ov<int, 1>, Ov<const char*, 2>> a;
  return a(5) + a("x") == 3 ? 0 : 1;
}
