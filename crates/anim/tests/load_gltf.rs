//! The skinned test model (tools/blender/make_skin_test.py) loads, and its clip bends it.

use glam::Vec3;
use std::path::Path;

#[test]
fn the_skin_test_model_loads_and_bends() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cache/skin_test/skin_test.gltf");
    if !p.exists() {
        eprintln!("(no {}: run tools/blender/make_skin_test.py)", p.display());
        return;
    }
    let l = anim::load(&p).unwrap();
    assert_eq!(l.skeleton.joints.len(), 3, "{:?}", l.skeleton.joints.iter().map(|j| &j.name).collect::<Vec<_>>());
    assert!(!l.primitives.is_empty());
    let clip = l.clips.iter().find(|c| c.name.contains("sway")).expect("the sway clip");
    assert!((clip.duration - 2.0).abs() < 0.05, "duration {}", clip.duration);
    let mesh = &l.primitives[0].mesh;
    let mut rest = anim::Skinned::default();
    mesh.skin(&l.skeleton.skin_matrices(&l.skeleton.rest_pose()), &mut rest);
    let err = rest.positions.iter().zip(&mesh.positions).map(|(a, b)| (Vec3::from(*a) - Vec3::from(*b)).length()).fold(0.0f32, f32::max);
    assert!(err < 1e-3, "rest pose moves the mesh by {err}");
    let mut bent = anim::Skinned::default();
    mesh.skin(&l.skeleton.skin_matrices(&clip.sample(&l.skeleton, 0.5, true)), &mut bent);
    let moved = bent.positions.iter().zip(&rest.positions).map(|(a, b)| (Vec3::from(*a) - Vec3::from(*b)).length()).fold(0.0f32, f32::max);
    assert!(moved > 0.3, "the top moved {moved} m");
}
