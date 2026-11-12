// [class.pre]/2, [conv.ptr]/3, [temp.constr.decl], [module.interface],
// spec/modules.md §11.11 "Forms carried by WP-1360": an importer's errors
// against a class named in its own base clause and a checked concept
// built from the declaration image are the ones against the replayed
// source.
//@ expect: compile-fail
//@ std: c++26
//@ module: imgout=Inputs/img-outside-refs.cppm
//@ strict-diags
//@ diag: 16:16 error "cannot initialize a variable of type 'ou::derived *' with an rvalue of type 'ou::base<ou::other> *'"
//@ diag: 17:3 error "no matching function for call to 'need'"
//@ diag: 18:17 error "static assertion failed due to requirement 'noexcept(ou::fn::go(1, 2))'"
import imgout;
struct S {};
void f() {
  ou::derived *p = static_cast<ou::base<ou::other> *>(nullptr);
  ou::need(S{});
  static_assert(noexcept(ou::fn::go(1, 2)));
}
