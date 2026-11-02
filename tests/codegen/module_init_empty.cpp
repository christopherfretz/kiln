// Itanium ABI §3.2.6: an interface unit with nothing to initialize still
// defines _ZGIW, with an empty body and no guard variable.
// (clang 19 gives _ZGIW no function attributes; kiln follows clang 22.)
//@ min-oracle: 22
//@ module: mod_init_c=Inputs/mod_init_c.cppm
export module e;
import mod_init_c;
export int fe() { return fc(); }
