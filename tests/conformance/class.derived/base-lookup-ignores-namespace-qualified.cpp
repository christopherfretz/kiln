// [class.derived.general]/2: lookup for a base class name considers only
// types, also when the name is a typedef, is qualified, or is found in an
// enclosing namespace past a namespace of the same name (clang 22, issue
// 16855). WP-958 sibling of base-lookup-ignores-namespace.cpp.
//@ std: c++20
//@ requires: clang>=22
//@ expect: compile-ok
struct x {};
typedef x xt;
namespace outer {
namespace xt {}
struct a : xt {};
struct b : ::xt {};
} // namespace outer
namespace q {
struct n {};
namespace inner {
namespace n {}
struct c : n {};
} // namespace inner
} // namespace q
