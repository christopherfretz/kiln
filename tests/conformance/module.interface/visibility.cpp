// [module.interface]/2, [module.global.frag]/3-4, [basic.lookup.argdep]/4:
// non-exported declarations of m (module linkage), m's global module
// fragment, and names only reachable through m are not visible to an
// importer, and ADL does not find a non-exported function of m's
// interface (as clang 22). clang 22 words the second and third errors
// "declaration of 'Hidden' must be imported from module 'm' before it is
// required" / "missing '#include'; 'gmf_helper' must be declared before
// it is used"; kiln reports plain lookup failures (follow-up).
//@ expect: compile-fail
//@ std: c++20
//@ args: -fmodule-file=m=%{test_dir}/Inputs/m.cppm
//@ skip(clang): clang needs a two-step --precompile build of Inputs/m.cppm that the harness cannot drive
//@ strict-diags
//@ diag: 22:9 error "use of undeclared identifier 'secret'"
//@ diag: 23:9 error "no type named 'Hidden' in namespace 'detail'"
//@ diag: 25:9 error "use of undeclared identifier 'gmf_helper'"
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
