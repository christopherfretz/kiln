// [temp.explicit]/10, [temp.point]: an explicit instantiation definition
// written inside a namespace (linkage specification, export) block is
// emitted before the block's own declarations, wherever in the block it
// is written: clang hands a top-level block to `HandleTopLevelDecl` once
// it is closed, but the explicit instantiation's members (and a variable
// template specialization instantiated at a reference) as soon as they
// are instantiated, while the block is parsed. Several in one block keep
// their order; a reopened block is a new top-level declaration. The
// vtables of explicitly instantiated classes and those of key functions
// are defined in one order, where each is used (clang's `VTableUses`:
// the explicit instantiation, the key function's definition). Found in
// downstream code; spec/landed/codegen/WP-1430.md. Static data members
// and implicit declarations: see the end.
//@ expect: ir
namespace n {
template <class T> struct B {
  int f() { return 1; }
  static int s;
};
template <class T> int B<T>::s = 3;
template <class T> int g(T) { return 2; }
template <class T> constexpr int cv = sizeof(T);
extern template struct B<long>;
int a() { return 3; }
template struct B<int>;
int b() { return 4; }
template int g<long>(long);
namespace k {
int e() { return 7; }
int h() { return 8; }
} // namespace k
template int g<char>(char);
int c() { return cv<short>; }
} // namespace n
int z() { return 0; }
namespace n {
int y() { return 1; }
template struct B<short>;
} // namespace n
extern "C++" {
int la() { return 3; }
template struct n::B<char>;
int lb() { return n::cv<int>; }
}
namespace n {
int nc() { return cv<char>; }
inline namespace i {
int d() { return 1; }
}
template struct B<long>;
int ne() { return B<unsigned>::s; }
} // namespace n
namespace p {
template <class T> struct D {
  virtual int v() { return 1; }
};
template <class T> struct E : D<T> {
  int v() override { return 3; }
};
struct K {
  virtual int k();
};
extern template struct D<short>;
template struct E<int>;
} // namespace p
template struct p::D<char>;
int p::K::k() { return 0; }
template struct p::D<short>;
struct L {
  virtual int l();
};
namespace p {
struct M {
  virtual int m();
};
int M::m() { return 1; }
template struct E<long>;
} // namespace p
int L::l() { return 2; }
template struct p::E<char>;
// A static data member's instantiated definition is emitted at its
// explicit instantiation (or its class's), not with the pattern's block.
int s0 = 0;
template int n::B<double>::s;
int s1 = 1;
namespace n {
template <class T> int vt = 4;
template int B<float>::s;
template int vt<long>;
} // namespace n
int s2 = n::vt<int>;
template struct n::B<unsigned char>;
// An implicit declaration (the global `operator delete` a virtual
// destructor declares) does not move the next explicit instantiation's
// vtable.
template <class T> struct V {
  virtual ~V() {}
};
struct W {
  virtual int w();
};
int W::w() { return 0; }
template struct V<int>;
template struct V<char>;
int s3 = 3;
// An explicitly specialized static data member is an ordinary definition,
// emitted where it is written, before its class's explicit instantiation.
namespace n {
template <> int B<wchar_t>::s = 6;
}
int s4 = 4;
template struct n::B<wchar_t>;
