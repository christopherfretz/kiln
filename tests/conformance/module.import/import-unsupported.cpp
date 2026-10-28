// [module.import], [cpp.import]: kiln does not implement modules yet; an
// import declaration is diagnosed rather than misparsed, and the rest of
// the translation unit is still checked.
//@ expect: compile-fail
//@ skip(clang): clang diagnoses a missing module instead
//@ strict-diags
//@ diag: 8:1 error "a module import is not supported yet"
import kiln.not_a_module;
int ok;
static_assert(sizeof(ok) == sizeof(int));
