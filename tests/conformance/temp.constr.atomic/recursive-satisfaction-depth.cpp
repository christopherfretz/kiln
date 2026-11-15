// [temp.constr.atomic]/3, [temp.inst]/16, [implimits]: a satisfaction
// check whose atomic constraint names a specialization constrained on the
// same concept with a new argument recurses through ever new
// specializations; clang counts each satisfaction check toward the
// instantiation depth limit, so it stops with the fatal depth error
// rather than running until memory or time runs out. The concept and the
// function share one line: kiln reports at the call in the concept, clang
// at the function's requires-clause. Found running GCC's C++ testsuite
// (WP-1456).
//@ std: c++20
//@ expect: compile-fail
//@ mem<=2048
//@ args: -ftemplate-depth=64
//@ diag: 16 fatal "recursive template instantiation exceeded maximum depth of 64"
namespace q { struct S {}; }
namespace q { template <int N, class T> concept Deep = requires(T t) { g<N + 1>(t); }; template <int N, class T> requires Deep<N, T> int g(T) { return N; } }
int h() { return g<0>(q::S{}); }
