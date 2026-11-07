// P2552R3 ([dcl.attr.grammar], a DR): __has_cpp_attribute reports a
// standard attribute only when the implementation gives it its
// recommended semantics; kiln reports the same set as clang.
//@ expect: compile-ok
//@ std: c++26
//@ strict-diags
static_assert(__has_cpp_attribute(no_unique_address));
static_assert(__has_cpp_attribute(nodiscard) == 201907L);
static_assert(__has_cpp_attribute(deprecated));
static_assert(__has_cpp_attribute(fallthrough));
static_assert(__has_cpp_attribute(maybe_unused));
static_assert(__has_cpp_attribute(likely));
static_assert(__has_cpp_attribute(unlikely));
static_assert(__has_cpp_attribute(noreturn));
static_assert(__has_cpp_attribute(assume));
struct E {};
struct S { [[no_unique_address]] E e; int i; };
static_assert(sizeof(S) == sizeof(int));
[[nodiscard("reason")]] int f();
