// Primary interface of module `p` ([module.unit]/3): re-exports its
// interface partition and declares a function defined in an
// implementation unit.
export module p;
export import :part;
import :impl;
export int p_total();
export int p_internal() { return impl_value(); }
