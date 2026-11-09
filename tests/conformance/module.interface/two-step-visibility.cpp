// [module.interface]/2, spec/modules.md §11.7: visibility.cpp with m's
// interface read from a module file built by a separate `--precompile`
// step (kiln: its source capsule) rather than from its source. The
// importer sees exactly what it saw in-process.
//@ expect: compile-fail
//@ std: c++20
//@ module: m=Inputs/m.cppm
//@ two-step
//@ min-oracle: 22
//@ strict-diags
//@ diag: 22:9 error "use of undeclared identifier 'secret'"
//@ diag: 23:9 error "declaration of 'Hidden' must be imported from module 'm' before it is required"
//@ diag: 25:9 error "missing '#include'; 'gmf_helper' must be declared before it is used"
//@ diag: 26:9 error "use of undeclared identifier 'frob'"
//
//
//
import m;
auto ok = make_hidden();
int d = ns::counter + limit + twice(1) + sq(2) + ok.v + use_gmf(1);
Point pt{1, 2};
int a = secret();
detail::Hidden h;
int ok2 = pt.sum();
int b = gmf_helper(1);
int c = frob(pt);
