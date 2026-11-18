// An inline static data member with a dynamic initializer, defined in its
// class in module `im` and odr-used in three units (im itself, this
// importer and Inputs/im-member-user.cpp), is initialized once: every
// unit's copy of the initialization is comdat-keyed and tests the same
// guard variable ([basic.start.dynamic]/1, [basic.def.odr]/14; WP-1653).
//@ expect: run
//@ std: c++20
//@ module: im=Inputs/im-member.cppm
//@ unit: Inputs/im-member-user.cpp
//@ stdout: 7 9 8 calls 1
import im;
extern "C" int printf(const char *, ...);

int user_value();
int mine = A::x + 1;

int main() { printf("%d %d %d calls %d\n", module_x(), user_value(), mine, init_calls()); }
