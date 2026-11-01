// [class.mem.general]/8, [basic.lookup.qual]: a class may be named in a
// nested-name-specifier while it is being defined; lookup finds the
// members declared so far. Here `C::duration` is formed by substituting
// a default template argument inside `C` (libc++ `system_clock`:
// `typedef chrono::time_point<system_clock> time_point;`).
template <class C, class D = typename C::duration> struct time_point { typedef D duration; };
struct clk {
  typedef long duration;
  typedef time_point<clk> tp;
};
static_assert(sizeof(clk::tp::duration) == sizeof(long));
