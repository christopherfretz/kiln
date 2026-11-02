// Itanium ABI §3.2.6, [basic.start.dynamic]/5: a module interface unit
// defines the guarded external initializer _ZGIW1m (an llvm.global_ctors
// entry instead of _GLOBAL__sub_I): exported imports' initializers first,
// then the other imports', then its own ordered initializers; an import
// whose initializer does nothing (mod_init_c) is not called.
// (clang 19 gives _ZGIW no function attributes; kiln follows clang 22.)
//@ min-oracle: 22
//@ module: mod_init_a=Inputs/mod_init_a.cppm
//@ module: mod_init_c=Inputs/mod_init_c.cppm
//@ module: mod_init_b=Inputs/mod_init_b.cppm
export module m;
import mod_init_a;
import mod_init_c;
export import mod_init_b;
export int vm = vb + va + fc();
int km = 4;
