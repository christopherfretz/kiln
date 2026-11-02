// [basic.stc.thread], [module.interface]/2, Itanium ABI §3.4: an
// interface unit defines its exported and module-linkage thread_local
// variables with their __tls_init/_ZTH aliases and weak_odr _ZTW
// wrappers (the wrapper of a variable of an importer reaches them). No
// function here uses a variable: clang emits a module unit's variable
// initializers after its functions, which kiln does not model.
//@ min-oracle: 22
module;
int f();
export module mtl;
export struct D { D(); ~D(); int v; };
export thread_local int x = f();
export thread_local int k = 3;
export thread_local D dd;
thread_local int y = f();
