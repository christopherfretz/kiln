// [temp.explicit]/10, [temp.expl.spec]: after an explicit instantiation
// declaration, the explicit instantiation definition in a reopened
// namespace defines the members of the class once; the explicitly
// specialized member keeps its own body, and virtual functions,
// constructors, destructors, nested classes and static data members are
// defined once each. Found building a large open-source code base
// (spec/landed/codegen/WP-1425.md).
//@ expect: run
//@ args: -g
//@ stdout: ctor spec s 5 ctor generic 2 v 11 n 4
//@ stdout: dtor
//@ stdout: dtor
extern "C" int printf(const char *, ...);
namespace n {
template <class T> struct A {
  A() { printf("ctor "); }
  ~A() { printf("dtor\n"); }
  virtual int v() { return 11; }
  int f() { return 2; }
  int which() { return 0; }
  static int s;
  struct N { int h() { return 4; } };
};
template <class T> int A<T>::s = 5;
template <> inline int A<long>::which() { printf("spec "); return 1; }
template <> inline int A<int>::which() { printf("never"); return 2; }
extern template struct A<long>;
} // namespace n
namespace n { template struct A<long>; }
namespace m { int z() { return 9; } }
template int n::A<int>::f();
int main() {
  n::A<long> a;
  a.which();
  printf("s %d ", n::A<long>::s);
  n::A<int> b;
  printf("generic %d", b.f());
  printf(" v %d", a.v());
  printf(" n %d\n", n::A<long>::N().h());
}
