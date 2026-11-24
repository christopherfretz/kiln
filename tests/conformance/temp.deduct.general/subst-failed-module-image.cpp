// [temp.deduct.general]/8 through a module's declaration image
// (`-fkiln-bmi-image`): the image carries no silent-failure mark (it holds
// no implicit specialization and no invalid declaration); the importer
// forms the specializations itself, and the ones that fail silently carry
// their marks (spec/modules.md "Inventory", spec/ast.md §4.1 "Silent
// failures"): the checker (`-fkiln-verify-subst-failed`) reports an
// invalid declaration without one as an error, so the compile succeeds
// only when the importer marked them. The marks themselves are checked by
// subst-failed-dump.cpp, without a module.
//@ module: subst_failed_mod=Inputs/subst_failed_mod.cppm
//@ two-step
//@ args: -fkiln-bmi-image -fkiln-verify-subst-failed
//@ skip(clang): kiln-only flags
//@ std: c++26
//@ expect: compile-ok
import subst_failed_mod;
int a = sf_g(1);
long b = sf_k(1);
