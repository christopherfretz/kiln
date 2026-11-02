// Itanium ABI §3.2.6, [module.unit]/3: a module partition defines its
// own initializer _ZGIW<module>WP<partition>.
// (clang 19 gives _ZGIW no function attributes; kiln follows clang 22.)
//@ min-oracle: 22
//@ module: mod_init_a=Inputs/mod_init_a.cppm
export module mp:part;
import mod_init_a;
export int vp = va + 2;
