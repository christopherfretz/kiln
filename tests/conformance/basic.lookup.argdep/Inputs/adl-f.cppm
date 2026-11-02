// Module for ../module-enum.cpp: imports E without exporting it.
export module F;
import E;
export P::En get_en() { return P::En::a; }
export P::Cl get_cl() { return {}; }
