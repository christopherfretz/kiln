// [basic.start.dynamic]/3: `A::x`, an inline static data member defined
// in the class in module `im`, has partially-ordered initialization, and
// its definition is appearance-ordered before `w`'s (this unit has an
// interface dependency on im), so it is initialized before `w`, whose
// dynamic initializer reads it, and before `seen`, whose initializer
// does not name it. The importer's own comdat-keyed copy of the guarded
// initialization precedes its ordered initializers, and the guard is
// shared with the module unit (WP-1653).
//@ expect: run
//@ std: c++20
//@ module: im=Inputs/im-member.cppm
//@ stdout: seen 1 w 8 via module 7 calls 1
import im;
extern "C" int printf(const char *, ...);

int seen = init_calls();
int w = A::x + 1;

int main() { printf("seen %d w %d via module %d calls %d\n", seen, w, module_x(), init_calls()); }
