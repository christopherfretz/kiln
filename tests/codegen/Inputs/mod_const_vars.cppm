// Module of module_const_var_importer: namespace-scope constants an
// importer can see the values of.
export module mod_const_vars;
struct LT { constexpr LT(int v) : v(v) {} int v; };
struct CD { constexpr CD(int v) : v(v) {} constexpr ~CD() {} int v; };
struct NT { constexpr NT(int v) : v(v) {} ~NT() {} int v; };
struct MU { constexpr MU(int v) : v(v) {} mutable int v; };
struct NAG { int v; ~NAG() {} };
export constexpr int c1 = 5;
export inline constexpr int c2 = 6;
export const int c3 = 7;
export constexpr const int *c4 = &c1;
export template <class T> constexpr T vt = T(3);
export template <> constexpr long vt<long> = 9;
export constexpr LT c5{8};
export const NT c6{9};
export constexpr int arr[3] = {1, 2, 3};
export extern const int c8;
const int c8 = 11;
export int g1 = 12;
export constexpr CD d1{4};
export constexpr MU d2{5};
export constexpr int &&d4 = 3;
export constexpr float d7 = 1.5f;
export constinit const int d9 = 6;
export volatile const int d10 = 7;
export const NAG e5{5};
export struct AG { int v; };
export const AG e6 = {6};
