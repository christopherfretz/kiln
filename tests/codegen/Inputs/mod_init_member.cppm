// Module of module_init_inline_member: its only dynamic initializers are
// inline static data members defined in their classes.
export module mod_init_member;
int next() { static int n; return ++n; }
export struct C {
  struct D { static inline int x = next(); };
};
export union U { static inline int y = next(); int m; };
struct Hidden { static inline int z = next(); };
export int hidden_z() { return Hidden::z; }
