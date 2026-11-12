// [expr.typeid]/3, [expr.const]: `typeid` of a glvalue of polymorphic
// class type evaluated in the Oven through functions the operand odr-uses
// (a constexpr member `operator*` and conversion function of class
// template specializations, a lambda's call operator); they are
// instantiated where the operand makes them potentially evaluated
// (WP-1322).
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ args: -Wno-potentially-evaluated-expression
#include <typeinfo>
struct CB { virtual constexpr ~CB() = default; };
struct CD : CB {};
template <class T> struct CP {
  T *p;
  constexpr T &operator*() const { return *p; }
};
template <class T> struct CW {
  T *p;
  constexpr operator T &() const { return *p; }
};
constexpr CD cd;
constexpr CP<const CB> cp{&cd};
constexpr CW<const CB> cw{&cd};
static_assert(typeid(*cp) == typeid(CD));
static_assert(typeid(static_cast<const CB &>(cw)) == typeid(CD));
constexpr bool via_lambda() {
  CD local;
  CP<CB> p{&local};
  return typeid(*p) == typeid(CD) &&
         typeid([&]() -> CB & { return *p; }()) == typeid(CD);
}
static_assert(via_lambda());
