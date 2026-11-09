// [module.interface]/2, [module.reach]/3, [std.modules]: only the names
// std.cppm exports are visible through `import std;`; libc++ internals are only reachable.
//@ expect: compile-fail
//@ std: c++20
//@ args: -stdlib=libc++ -Wno-reserved-module-identifier
//@ module: std=%{libcxx_modules}/std.cppm
//@ strict-diags
//@ diag: 11:15 error "missing '#include <__memory/pointer_traits.h>'; '__to_address' must be declared before it is used"
import std;
int x = 0;
auto p = std::__to_address(&x);
