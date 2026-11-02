// [module.interface]/2, [module.reach]/3, [std.modules]: only the names
// std.cppm exports are visible through `import std;`; libc++ internals are only reachable.
//@ expect: compile-fail
//@ std: c++20
//@ args: -stdlib=libc++
//@ strict-diags
//@ diag: 11:15 error "missing '#include <__memory/pointer_traits.h>'; '__to_address' must be declared before it is used"
//@ skip(clang): clang needs a prebuilt std BMI (two-step --precompile of libc++'s std.cppm) that the harness cannot drive
import std;
int x = 0;
auto p = std::__to_address(&x);
