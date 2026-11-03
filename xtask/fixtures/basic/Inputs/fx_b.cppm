// A module interface unit for pass-module.cpp that imports fx_a.
export module fx_b;
import fx_a;
export int fb() { return fa() + 1; }
