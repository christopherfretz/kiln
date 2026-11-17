// [basic.start.dynamic]/1, [dcl.inline]/6, Itanium ABI §2.8: an inline
// static data member with a dynamic initializer is initialized once in
// the program, although both the module interface unit and its importer
// emit its guarded initializer; the redundant redeclaration of a
// constexpr member adds no initializer (WP-1523).
//@ expect: run
//@ std: c++20
//@ module: inline_member_init_once=Inputs/inline_member_init_once.cppm
//@ stdout: init 1
//@ stdout: init k
//@ stdout: 7 1 2
extern "C" int printf(const char *, ...);
import inline_member_init_once;

int main() { printf("%d %d %d\n", touch() - A::k + 4, A::c.id, A::n); }
