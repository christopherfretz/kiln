// The written-reference table (spec/ast.md §18.4), role `type`: a
// type-name in every type-specifier context ([dcl.type.simple]):
// declarations, parameters, return types (also trailing), every kind of
// cast, new-expressions, sizeof/alignof/noexcept operands, template
// arguments, decltype operands, conversion-function-ids, exception
// handlers, typedef and alias targets, the last component of a
// typename-specifier, an elaborated-type-specifier that refers to an
// existing class, an injected-class-name, a using-enum operand and a
// pseudo-destructor name. A type found through a using-declaration has
// the using-shadow as `found`.
//@ written-refs: type-names.refs
namespace lib {
struct Rec {
  int n;
  operator int() const { return n; }
};
struct Err {};
enum class Mode { on, off };
typedef int Count;
template <class T> struct Box {
  T v;
  Box* self() { return this; }   // injected-class-name
};
} // namespace lib

namespace app {
using lib::Rec;
using Alias = lib::Rec;
typedef lib::Count Total;

Rec make(lib::Count c);
auto later(Rec r) -> lib::Count;
Total sum(Alias* a, int k) {
  struct lib::Rec* e = a;          // elaborated reference to an existing class
  Rec* p = new Rec{k};
  const Rec& q = static_cast<const Rec&>(*p);
  long x = reinterpret_cast<long>(p) + (long)const_cast<Rec*>(&q)->n;
  x += sizeof(Rec) + alignof(lib::Err) + noexcept(Rec{}) + int(Total(1));
  decltype(Rec{}.n) y = q.operator int();
  lib::Box<Rec> b{*e};
  delete p;
  try {
    throw lib::Err{};
  } catch (const lib::Err&) {
    y += 1;
  }
  return static_cast<Total>(x + y + b.v.n);
}

struct Probe {
  using enum lib::Mode;
  lib::Mode m = on;
};

template <class T> typename T::value_type first(const T& t) {
  typename T::value_type v = t.front();
  return v;
}

typedef int Int;
void destroy(Int* p) { p->~Int(); }
template <class T> void destroy_t(T* p) { p->~T(); }

struct Poly {
  virtual ~Poly();
  operator lib::Err() const;       // conversion-function-id with a class type
};
struct Leaf : Poly {};
lib::Err convert(Poly* p) {
  Leaf* l = dynamic_cast<Leaf*>(p);
  return l->operator lib::Err();
}
} // namespace app
