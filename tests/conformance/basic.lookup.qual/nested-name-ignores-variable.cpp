// [basic.lookup.qual]/1: lookup of the name before `::` considers only
// namespaces, types and templates whose specializations are types, so a
// variable does not hide a namespace (alias) or class there, also when the
// parser must decide whether `fs::path(...)` is a functional cast or an
// expression (libc++ ifstream.cons/path: `std::ifstream fs(fs::path(".."))`).
//@ expect: run
//@ strict-diags
namespace N {
struct path { int n; path(int n) : n(n) {} static int v; struct I { int x; }; };
int path::v = 4;
template <class T> struct X { using type = T; };
}
namespace fs = N;
struct S { struct T { int y; }; };
struct F { int n; explicit F(const N::path& p) : n(p.n) {} };
int main() {
  F fs(fs::path(1));
  int N = 0, s = 0;
  { int S = 0; S::T t{3}; s += t.y + S; }
  N::path::I i{N::path::v};
  N::X<int>::type b = 2;
  auto p = N::path(5);
  int N::path::*pm = &N::path::n;
  return fs.n + i.x + b + p.*pm + s + N == 1 + 4 + 2 + 5 + 3 ? 0 : 1;
}
