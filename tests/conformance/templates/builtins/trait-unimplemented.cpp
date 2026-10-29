// The traits that need overload resolution's unevaluated init_check /
// conv_check (spec/sema-templates.md §9.1, owner WP-068a) report
// sema_unimplemented_template_feature until then; so does a trait whose
// class property kiln does not compute yet. __has_builtin is 0 for them
// (has-builtin-staged-off.cpp), so libc++ does not use them.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(clang): clang 19 implements these traits
//@ diag: 17:10 error "not yet implemented: type trait '__is_constructible'"
//@ diag: 18:10 error "not yet implemented: type trait '__is_convertible'"
//@ diag: 19:10 error "not yet implemented: type trait '__is_nothrow_assignable'"
//@ diag: 20:10 error "not yet implemented: type trait '__reference_binds_to_temporary'"
//@ diag: 21:10 error "not yet implemented: type trait '__is_trivially_relocatable'"
//@ diag: 22:10 error "not yet implemented: type trait '__is_trivially_equality_comparable'"
//@ diag: 23:10 error "not yet implemented: type trait '__is_layout_compatible'"
struct S { int x; };
bool a = __is_constructible(S, int);
bool b = __is_convertible(int, long);
bool c = __is_nothrow_assignable(int &, int);
bool d = __reference_binds_to_temporary(const int &, long);
bool e = __is_trivially_relocatable(S);
bool f = __is_trivially_equality_comparable(S);
bool g = __is_layout_compatible(S, S);
