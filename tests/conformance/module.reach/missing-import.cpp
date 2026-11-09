// [module.reach]/1-3, [basic.lookup.general]/2, [module.interface]/2,
// [module.global.frag]/3: a declaration that is reachable from the
// importer but not visible (an exported declaration of a module imported
// only by an imported unit or partition, a module-linkage declaration
// named by a qualified name, a global module fragment declaration) is
// reported as clang 22 does, "declaration of 'X' must be imported from
// module 'M' before it is required" / "missing '#include'; 'X' must be
// declared before it is used", with a note "declaration here is not
// visible" at the declaration (in Inputs/, so not matched here). The use
// recovers with the declaration: an exported one makes its unit visible
// from then on, a global-module one is reported at every use. An
// unqualified name of a module-linkage declaration is a plain lookup
// failure (clang 22 reports it as a missing import too once a qualified
// lookup has loaded the namespace; kiln does not model that). The notes
// are checked by kiln_driver's `module_missing_import_notes` test.
//@ expect: compile-fail
//@ std: c++20
//@ module: mr.a=Inputs/mr-a.cppm
//@ module: mr.b:part=Inputs/mr-p.cppm
//@ module: mr.b=Inputs/mr-b.cppm
//@ requires: clang>=22
//@ strict-diags
//@ diag: 35:1 error "unknown type name 'LinkageGlobalType'"
//@ diag: 36:10 error "use of undeclared identifier 'linkage_other'"
//@ diag: 37:10 error "declaration of 'a_fn' must be imported from module 'mr.a' before it is required"
//@ diag: 39:10 error "declaration of 'part_fn' must be imported from module 'mr.b' before it is required"
//@ diag: 41:14 error "missing '#include'; 'gmf_qual' must be declared before it is used"
//@ diag: 42:10 error "missing '#include'; 'gmf_fn' must be declared before it is used"
//@ diag: 43:10 error "missing '#include'; 'gmf_fn' must be declared before it is used"
//@ diag: 44:14 error "declaration of 'linkage_fn' must be imported from module 'mr.b' before it is required"
//@ diag: 45:12 error "declaration of 'linkage_global' must be imported from module 'mr.b' before it is required"
//@ diag: 46:1 error "missing '#include'; 'GmfType' must be declared before it is used"
//@ diag: 47:5 error "declaration of 'LinkageType' must be imported from module 'mr.b' before it is required"
import mr.b;
LinkageGlobalType lgt;
int v0 = linkage_other();
int v1 = a_fn();
AType at;
int v2 = part_fn();
int v3 = b_fn() + en::exported_fn();
int v4 = gn::gmf_qual();
int v5 = gmf_fn();
int v6 = gmf_fn();
int v7 = en::linkage_fn();
int v8 = ::linkage_global();
GmfType gt;
en::LinkageType lt;
