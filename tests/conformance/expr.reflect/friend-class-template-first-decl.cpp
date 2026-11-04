// [expr.reflect], [namespace.memdef]/3, [temp.dep.constexpr]: a class
// template first declared by a friend declaration inside a class
// template is a member of the enclosing namespace. Reflecting it (also
// as a template argument of a reflected specialization) is not
// value-dependent, so the reflection is a constant in a non-template
// function (GCC 16.1 agrees).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta.h"
namespace n {
template <class V> class Node {
    template <class A, class B> friend class list;
};
template <class A, class B> class list;
template <class A, class B = int> class list {};
class Plain;
template <class V> class Holder {
    friend class Plain;
};
class Plain {};
}  // namespace n
template <std::meta::info T> struct caster_ {};
template <std::meta::info... Fs> struct config_ {};
consteval std::meta::info f() { return ^^config_<^^caster_<^^n::list>>; }
static_assert(f() == ^^config_<^^caster_<^^n::list>>);
consteval std::meta::info g() { return ^^n::list; }
static_assert(g() == ^^n::list);
consteval std::meta::info h() { return ^^caster_<^^n::Plain>; }
static_assert(h() == ^^caster_<^^n::Plain>);
static_assert(std::meta::template_arguments_of(f()).size() == 1);
