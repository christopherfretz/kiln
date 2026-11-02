// [basic.link]/8, [temp.arg.nontype]: a specialization whose class-type
// template argument names an entity with internal linkage — through a
// pointer, reference or member pointer member at any depth (a nested
// class, an array element, a base subobject, a union's active member) —
// is not usable from another translation unit; clang gives it internal
// linkage (`getLVForTemplateArgumentList` → `getLVForValue`): the
// function and variable templates, the members, vtable and RTTI of a
// class template specialization, and the template parameter object
// itself (still in its comdat under clang 19, without alignment).
// Arguments naming only externally visible entities stay `linkonce_odr`.
namespace {
int hidden;
int arr2[2];
void hf() {}
struct HS {
  int m;
};
} // namespace
static int shidden;
int visible;
void vf() {}
struct P {
  int *p;
};
struct Nested {
  int k;
  P inner;
};
struct Arr {
  int *ps[2];
};
struct Base {
  int *b;
};
struct Derived : Base {
  int d;
};
struct Ref {
  int &r;
};
struct FnP {
  void (*f)();
};
struct MemH {
  int HS::*mp;
};
union U {
  int i;
  int *p;
};
template <P V> int fp() { return 1; }
template <Nested V> int fn() { return 2; }
template <Arr V> int fa() { return 3; }
template <Derived V> int fd() { return 4; }
template <Ref V> int fr() { return 5; }
template <FnP V> int ff() { return 6; }
template <MemH V> int fm() { return 7; }
template <U V> int fu() { return 8; }
template <P V> struct C {
  int get() { return 9; }
  static int s;
  virtual ~C() {}
};
template <P V> int C<V>::s = 10;
template <P V> int vt = 11;
template <const P *Q> int ptpo() { return 12; }
template <P V> int viatpo() { return ptpo<&V>(); }
template <P V> const P *addrof() { return &V; }
int main() {
  int r = fp<P{&hidden}>() + fp<P{&shidden}>() + fp<P{&visible}>() + fp<P{nullptr}>();
  r += fn<Nested{1, P{&hidden}}>() + fn<Nested{1, P{&visible}}>();
  r += fa<Arr{{&visible, &hidden}}>() + fa<Arr{{&visible, &visible}}>();
  r += fa<Arr{{&arr2[1], nullptr}}>();
  r += fd<Derived{{&hidden}, 1}>() + fd<Derived{{&visible}, 2}>();
  r += fr<Ref{hidden}>() + fr<Ref{visible}>();
  r += ff<FnP{&hf}>() + ff<FnP{&vf}>();
  r += fm<MemH{&HS::m}>();
  r += fu<U{.p = &hidden}>() + fu<U{.i = 3}>();
  C<P{&hidden}> c1;
  C<P{&visible}> c2;
  r += c1.get() + c2.get() + C<P{&hidden}>::s + C<P{&visible}>::s;
  r += vt<P{&hidden}> + vt<P{&visible}>;
  r += viatpo<P{&hidden}>() + viatpo<P{&visible}>();
  return r + (addrof<P{&hidden}>() != addrof<P{&visible}>());
}
