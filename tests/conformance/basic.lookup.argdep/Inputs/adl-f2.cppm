// Module for ../module-enum-overload.cpp: imports E2 without exporting it.
export module F2;
import E2;
export P::Un get_un() { return P::a; }
