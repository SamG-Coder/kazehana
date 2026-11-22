//! Spring bones: a hanging chain lags when the body moves off, then settles back to its pose.

use anim::{Joint, Skeleton, Springs, Trs};
use glam::{Mat4, Vec3};

fn chain() -> Skeleton {
    let j = |name: &str, parent: Option<usize>, t: Vec3| Joint { name: name.into(), parent, rest: Trs { t, ..Default::default() } };
    let joints = vec![
        j("pelvis", None, Vec3::new(0.0, 1.0, 0.0)),
        j("spring_skirt0_0", Some(0), Vec3::ZERO),
        j("spring_skirt0_1", Some(1), Vec3::new(0.15, -0.2, 0.0)),
        j("spring_skirt0_end", Some(2), Vec3::new(0.05, -0.2, 0.0)),
    ];
    let n = joints.len();
    Skeleton { joints, inverse_bind: vec![Mat4::IDENTITY; n], root: Mat4::IDENTITY }
}

fn tip(skel: &Skeleton, pose: &anim::Pose, at: Mat4) -> Vec3 {
    let g = skel.globals(pose);
    (at * g[3]).w_axis.truncate()
}

#[test]
fn a_skirt_lags_behind_and_settles() {
    let skel = chain();
    let mut springs = Springs::new(&skel);
    assert!(!springs.is_empty());
    let dt = 1.0 / 60.0;
    let rest_tip = tip(&skel, &skel.rest_pose(), Mat4::IDENTITY);
    let mut x = 0.0;
    let mut pose = skel.rest_pose();
    // walk along +X at 1.4 m/s for half a second
    for _ in 0..30 {
        x += 1.4 * dt;
        pose = skel.rest_pose();
        springs.apply(&skel, &mut pose, Mat4::from_translation(Vec3::new(x, 0.0, 0.0)), dt);
    }
    let at = Mat4::from_translation(Vec3::new(x, 0.0, 0.0));
    let moving = tip(&skel, &pose, at) - Vec3::new(x, 0.0, 0.0);
    assert!(moving.x < rest_tip.x - 0.005, "the hem should trail behind while walking: {moving} vs {rest_tip}");
    // stop, and give it two seconds
    for _ in 0..120 {
        pose = skel.rest_pose();
        springs.apply(&skel, &mut pose, at, dt);
    }
    let still = tip(&skel, &pose, at) - Vec3::new(x, 0.0, 0.0);
    assert!((still - rest_tip).length() < 0.01, "the hem should settle back: {still} vs {rest_tip}");
    // the bones keep their lengths
    let g = skel.globals(&pose);
    let l1 = (g[3].w_axis - g[2].w_axis).truncate().length();
    assert!((l1 - Vec3::new(0.05, -0.2, 0.0).length()).abs() < 1e-4);
}

/// On a built character (when one is in the cache): her skirt's hem trails as she walks off and
/// settles when she stops, and nothing blows up.
#[test]
fn a_built_schoolgirl_s_skirt_swings() {
    let path = std::path::Path::new("../../assets/cache/people_review/springs/p519/p519.gltf");
    if !path.exists() {
        eprintln!("(no built character at {}: skipped)", path.display());
        return;
    }
    let loaded = anim::load(path).unwrap();
    let skel = &loaded.skeleton;
    let walk = loaded.clips.iter().find(|c| c.name.to_lowercase().contains("walk")).expect("a walk clip");
    let hem = skel.find("spring_skirt0_end").expect("skirt springs");
    let mut springs = Springs::new(skel);
    let dt = 1.0 / 60.0;
    let (mut x, mut t) = (0.0f32, 0.0f32);
    let mut most = 0.0f32;
    for frame in 0..240 {
        let walking = frame < 120;
        if walking {
            x += 1.3 * dt;
        }
        t += dt;
        let mut pose = walk.sample(skel, t, true);
        let at = Mat4::from_translation(Vec3::new(0.0, 0.0, -x));
        springs.apply(skel, &mut pose, at, dt);
        let plain = walk.sample(skel, t, true);
        let g = skel.globals(&pose);
        let g0 = skel.globals(&plain);
        let p = (at * g[hem]).w_axis.truncate();
        let p0 = (at * g0[hem]).w_axis.truncate();
        assert!(p.is_finite());
        let off = (p - p0).length();
        if walking {
            most = most.max(off);
        } else if frame > 230 {
            assert!(off < 0.03, "settled within 3 cm of the animation: {off}");
        }
    }
    assert!(most > 0.005 && most < 0.25, "the hem swings a little while walking: {most} m");
    println!("skirt hem swing while walking: {:.1} cm", most * 100.0);
}
