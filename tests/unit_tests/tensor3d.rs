use zahirscan::parsers::structured::tensor3d::{
    stack_axis_preferred, tensor3d_max_reported_planes, unravel_c_3d,
};

#[test]
fn unravel_c_matches_linear() {
    // 2*3*4, C, idx = i0*12 + i1*4 + i2
    for idx in 0..24 {
        let (a, b, c) = unravel_c_3d(idx, 2, 3, 4);
        assert_eq!(idx, a * 12 + b * 4 + c);
    }
}

#[test]
fn stack_axis_smallest_dim_tie_lowest_index() {
    assert_eq!(stack_axis_preferred(10, 20, 5, false), 2);
    assert_eq!(stack_axis_preferred(5, 5, 10, false), 0);
}

#[test]
fn stack_axis_equal_dims_uses_contiguous_fallback() {
    assert_eq!(stack_axis_preferred(4, 4, 4, false), 0);
    assert_eq!(stack_axis_preferred(4, 4, 4, true), 2);
}

#[test]
fn reported_planes_scales_by_decade_above_1e3() {
    assert_eq!(tensor3d_max_reported_planes(0), 0);
    assert_eq!(tensor3d_max_reported_planes(1_000), 32);
    assert_eq!(tensor3d_max_reported_planes(10_000), 35);
    assert_eq!(tensor3d_max_reported_planes(100_000), 38);
    assert_eq!(tensor3d_max_reported_planes(1_000_000), 41);
    assert_eq!(tensor3d_max_reported_planes(1_000_000_000_000_000_000), 64);
}
