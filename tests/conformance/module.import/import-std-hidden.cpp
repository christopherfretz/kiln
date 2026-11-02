// [module.interface]/2, [std.modules]: only the names std.cppm exports
// are visible through `import std;`; libc++'s internal helpers are not.
//@ expect: compile-fail
//@ std: c++20
//@ args: -stdlib=libc++
//@ strict-diags
//@ diag: 11:15 error "no member named '__to_address' in namespace 'std'"
//@ skip(clang): clang needs a prebuilt std BMI (two-step --precompile of libc++'s std.cppm) that the harness cannot drive
import std;
int x = 0;
auto p = std::__to_address(&x);
