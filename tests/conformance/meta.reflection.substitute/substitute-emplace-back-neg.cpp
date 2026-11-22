// [meta.reflection.substitute], [dcl.spec.auto.general] and its
// note, [temp.deduct.general] (P2996R13): an emplace_back-style member
// template (a defaulted element type, a parameter pack, a decltype(auto)
// return type, a static_assert on the element type and then an allocation
// of it), probed with can_substitute and no arguments: the definition of
// emplace_back<void> is instantiated to deduce the return type, and both
// errors in it are hard errors, not a false answer. Reduced from real code.
// GCC 16.1 answers true without instantiating it (WP-1929).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 does not instantiate a member function template to deduce its return type in can_substitute (WP-1929)
//@ strict-diags
//@ diag: 26:19 error "static assertion failed due to requirement '!std::is_void_v<void>': emplacement type must not be void"
//@ diag: 32:25 note "in instantiation of function template specialization 'array::emplace_back<void>' requested here"
//@ diag: 32:25 note "while substituting template arguments into 'array::emplace_back' for std::meta::substitute"
//@ diag: 27:16 error "allocation of incomplete type 'void'"
#include <meta>
#include <type_traits>

struct node { virtual ~node() = default; };
struct array {
  node* last = nullptr;
  template <typename E = void, typename... Args>
  decltype(auto) emplace_back(Args&&... args) {
    static_assert(!std::is_void_v<E>, "emplacement type must not be void");
    E* p = new E(static_cast<Args&&>(args)...);
    last = p;
    return *p;
  }
};
constexpr bool probed = std::meta::can_substitute(^^array::emplace_back, {});

int main() {}
