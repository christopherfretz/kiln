// [module.interface]/2, [module.global.frag]/3-4, [basic.lookup.argdep]/4:
// non-exported declarations of m (module linkage), m's global module
// fragment, and names only reachable through m are not visible to an
// importer, and ADL does not find a non-exported function of m's
// interface (as clang 22). A qualified name of a non-exported declaration
// and a name of a global module fragment declaration are reported as
// missing imports, with a note at the declaration
// (module.reach/missing-import.cpp).
//@ expect: compile-fail
//@ std: c++20
//@ module: m=Inputs/m.cppm
//@ requires: clang>=22
//@ strict-diags
//@ diag: 22:9 error "use of undeclared identifier 'secret'"
//@ diag: 23:9 error "declaration of 'Hidden' must be imported from module 'm' before it is required"
//@ diag: 25:9 error "missing '#include'; 'gmf_helper' must be declared before it is used"
//@ diag: 26:9 error "use of undeclared identifier 'frob'"
import m;
auto ok = make_hidden();
int d = ns::counter + limit + twice(1) + sq(2) + ok.v + use_gmf(1);
Point pt{1, 2};
int a = secret();
detail::Hidden h;
int ok2 = pt.sum();
int b = gmf_helper(1);
int c = frob(pt);
