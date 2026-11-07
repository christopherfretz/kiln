// [expr.cast]/4: a C-style or functional cast performs a static_cast
// between a derived class and its base even when the base is
// inaccessible (private or protected, virtual or not), pointers and
// references both. In a SFINAE context the cast is therefore valid, which
// is how Boost.TypeTraits `is_virtual_base_of` tells a virtual base (no
// downcast) from a non-virtual one.
//@ expect: run
//@ stdout: 1 1 1 0 | 2 2 2 2 | 1 3
extern "C" int printf(const char*, ...);

template <class T> T* dv();
template <class> struct void_t_ { typedef void type; };
template <class B, class D> constexpr bool no_downcast(...) { return true; }
template <class B, class D,
          typename void_t_<decltype((D*)(dv<B>()))>::type* = nullptr>
constexpr bool no_downcast(int) { return false; }

struct B { int b = 2; };
struct Pad { int pad = 9; };
struct Priv : private virtual B {};
struct Prot : Pad, protected virtual B {};
struct NV : Pad, private B { int d = 3; };
typedef B* BP;

int main() {
  constexpr bool a = no_downcast<B, Priv>(0), b = no_downcast<B, Prot>(0),
                 c = !no_downcast<Priv, B>(0), d = no_downcast<B, NV>(0);
  printf("%d %d %d %d | ", a, b, c, d);
  Priv pv;
  Prot pt;
  NV nv;
  B* x = (B*)&pv;
  B& y = (B&)pt;
  B* z = BP(&nv);
  const B& w = (const B&)nv;
  printf("%d %d %d %d | ", x->b, y.b, z->b, w.b);
  NV* back = (NV*)z;
  printf("%d %d\n", back == &nv, back->d);
}
