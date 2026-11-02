// Implementation unit of `p` ([module.unit]/8: implicitly imports p).
module p;
int p_total() { return part_value() + p_internal(); }
