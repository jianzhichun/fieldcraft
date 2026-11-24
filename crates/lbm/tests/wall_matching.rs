//! The matched on-node wall (`wall_model: "spalding_bounce_matched"`, `crates/lbm/src/bounce.rs`,
//! `crates/lbm/src/wall.rs`): Han, Ooka and Kikumoto's eq. 14 bounce with Spalding's law read
//! [`MATCHING_ROWS`] cells from the wall, and Malaspinas and Sagaut's eq. 57 mixing-length eddy
//! viscosity on the rows below it. The law's reading, the rows' relaxation rates, the refusals,
//! the unchanged `spalding_bounce` and the determinism.

mod common;

use common::*;
use fieldcraft_lbm::bounce::MATCHING_ROWS;
use fieldcraft_lbm::lattice::{C, Q};
use fieldcraft_lbm::{Flow, Limits, Setup};
use fieldcraft_model::{Collision, FaceCondition, HrrSigma, Procedure, Turbulence, WallModel};

const PERIODIC: FaceCondition = FaceCondition::Periodic {};
/// Han et al.'s constants (their eqs. 16–17) and Malaspinas and Sagaut's (eq. 57–58).
const KAPPA: f64 = 0.41;
const B: f64 = 5.5;
const MIXING_KAPPA: f64 = 0.384;
const MIXING_A_PLUS: f64 = 26.0;

/// A turbulent step with Smagorinsky's model and the on-node bounce with `wall_model`.
fn bounced_with(wall_model: WallModel, van_driest: Option<f64>, end_time: f64, time_step: f64, collision: Collision) -> Procedure {
    let mut p = collide(end_time, time_step, None, collision);
    if let Procedure::Fluid { turbulence, sigma, .. } = &mut p {
        *turbulence = Some(Turbulence::Smagorinsky { constant: 0.1, prandtl: 0.9, van_driest, wall_model: Some(wall_model) });
        if collision == Collision::Hrr {
            *sigma = Some(HrrSigma::Value(0.99));
        }
    }
    p
}

/// The perturbed coarse channel of `wall_bounce.rs` (6 cells per half-height, 13 across) with
/// the matched wall.
fn matched_channel(collision: Collision, steps: f64) -> (Setup, Flow) {
    channel_with(WallModel::SpaldingBounceMatched, collision, steps)
}

/// The channel of `wall_bounce.rs`'s `perturbed_on_node_channel` with `wall_model`.
fn channel_with(wall_model: WallModel, collision: Collision, steps: f64) -> (Setup, Flow) {
    use std::f64::consts::PI;
    let (n, re_tau, ub) = (6usize, 180.0, 15.7);
    let dx = 1.0 / n as f64;
    let (lx, lz) = ((2.0 * PI * n as f64).round() * dx, (PI * n as f64).round() * dx);
    let mut d = domain([0.0; 3], [lx, 2.0 + dx, lz], dx);
    for f in [&mut d.faces.x_min, &mut d.faces.x_max, &mut d.faces.z_min, &mut d.faces.z_max] {
        *f = PERIODIC;
    }
    d.acceleration = [1.0, 0.0, 0.0];
    let dt = 0.1 * dx / ub;
    let (setup, mut flow) = prepare(&model(1.0, 1.0 / re_tau, d, bounced_with(wall_model, None, steps * dt, dt, collision)), &[]);
    let grid = *flow.grid();
    assert_eq!(grid.cells[1], 2 * n + 1);
    let to_lattice = 1.0 / setup.velocity_scale();
    let (kx, kz) = (2.0 * PI / lx, 2.0 * PI / lz);
    flow.set_equilibrium(|cell| {
        let [x, y, z] = grid.centre(cell);
        // The walls at y = Δx/2 and 2 + Δx/2.
        let eta = y - 1.0 - 0.5 * dx;
        let wall = (1.0 - eta * eta).max(0.0);
        let u = 1.15 * ub * (1.0 - eta.abs().min(1.0).powi(8)) + 2.0 * (2.0 * kz * z).sin() * (kx * x).cos() * wall;
        let v = 1.5 * wall * (2.0 * kz * z).cos() * (3.0 * kx * x).sin();
        let w = 1.5 * wall * eta * (kx * x).sin() * (kz * z).cos();
        (1.0, [u * to_lattice, v * to_lattice, w * to_lattice])
    });
    (setup, flow)
}

/// The cell `rows` cells from `cell` along the inward normal of a wall of `axis`.
fn along(flow: &Flow, cell: usize, axis: usize, inward: i32, rows: usize) -> usize {
    let mut ijk = flow.grid().coords(cell);
    ijk[axis] = (ijk[axis] as i64 + i64::from(inward) * rows as i64) as usize;
    flow.grid().index(ijk)
}

/// The fluid's momentum and mass: its cells' populations, and the populations that have
/// arrived at the boundary nodes from the fluid and not yet gone back.
fn fluid_momentum_mass(flow: &Flow) -> ([f64; 3], f64) {
    let (mut p, mut m) = ([0.0; 3], 0.0);
    let mut add = |cell: usize, q: usize| {
        let f = flow.population(q, cell);
        m += f;
        for a in 0..3 {
            p[a] += f64::from(C[q][a]) * f;
        }
    };
    for cell in (0..flow.grid().len()).filter(|c| flow.is_fluid(*c)) {
        for q in 0..Q {
            add(cell, q);
        }
    }
    for w in flow.wall_nodes().unwrap().nodes() {
        for q in (0..Q).filter(|q| C[*q][usize::from(w.axis)] * i32::from(w.inward) < 0) {
            add(w.cell as usize, q);
        }
    }
    (p, m)
}

/// Spalding's y⁺(u⁺) with Han et al.'s constants.
fn spalding_y_plus(up: f64) -> f64 {
    let k = KAPPA * up;
    up + (-KAPPA * B).exp() * (k.exp() - 1.0 - k - k * k / 2.0 - k * k * k / 6.0)
}

/// u⁺ at `y_plus` by bisection on [`spalding_y_plus`], and its slope du⁺/dy⁺ there.
fn spalding_u_plus(y_plus: f64) -> (f64, f64) {
    let (mut lo, mut hi) = (0.0, y_plus);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if spalding_y_plus(mid) < y_plus {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let u = 0.5 * (lo + hi);
    let k = KAPPA * u;
    let e = (-KAPPA * B).exp();
    let dy_du = 1.0 + KAPPA * e * (k.exp() - 1.0 - k - k * k / 2.0);
    (u, 1.0 / dy_du)
}

/// Each node's law is read [`MATCHING_ROWS`] cells from the wall: in the perturbed coarse channel,
/// step by step from the start, on every collision, the sample cell's (ρ, u) before the step give
/// the stress the walls take, and its friction velocity u_τ satisfies Spalding's law at that
/// distance, y = [`MATCHING_ROWS`] Δx; the walls take the summed stress (x and z, to round-off),
/// the fluid's momentum budget closes and its mass is conserved.
#[test]
fn the_law_is_read_three_cells_from_the_wall() {
    for collision in [Collision::Trt, Collision::Regularized, Collision::Hrr] {
        let (setup, mut flow) = matched_channel(collision, 40.0);
        assert!(flow.on_node_bounce() && !flow.wall_bounce() && !flow.wall_model());
        let a = flow.acceleration();
        let nu = (setup.tau_plus - 0.5) / 3.0;
        let (mut p, m0) = fluid_momentum_mass(&flow);
        let (mut budget, mut carried, mut law, mut mass, mut shear) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for _ in 0..setup.steps {
            let before: Vec<Option<(f64, [f64; 3])>> = (0..flow.grid().len()).map(|c| flow.moments(c)).collect();
            let bulk = flow.mass();
            flow.step().unwrap();
            let (next, m) = fluid_momentum_mass(&flow);
            let nodes = flow.wall_nodes().unwrap().nodes();
            let wall: [f64; 3] = [0, 1, 2].map(|k| flow.forces().iter().map(|f| f[k]).sum());
            let stress: [f64; 3] = [0, 1, 2].map(|k| nodes.iter().map(|w| w.stress[k]).sum());
            // Each boundary node's half-cell body force, ½ ρ a, as the step applied it (the walls' share).
            let share: [f64; 3] = [0, 1, 2].map(|k| nodes.iter().map(|w| w.body[k]).sum());
            for k in 0..3 {
                budget = budget.max((next[k] - p[k] - (a[k] * bulk + share[k] - wall[k])).abs() / next[0].abs());
            }
            for k in [0, 2] {
                carried = carried.max((wall[k] - stress[k]).abs() / stress[0].abs());
            }
            for w in nodes {
                let sample = along(&flow, w.cell as usize, usize::from(w.axis), i32::from(w.inward), MATCHING_ROWS);
                assert_eq!(sample, w.sample as usize);
                assert_eq!(along(&flow, w.cell as usize, usize::from(w.axis), i32::from(w.inward), 1), w.interior as usize);
                let (rho, u) = before[sample].unwrap();
                let speed = (u[0] * u[0] + u[2] * u[2]).sqrt();
                let s = (w.stress[0] * w.stress[0] + w.stress[2] * w.stress[2]).sqrt();
                let ut = (s / rho).sqrt();
                // Spalding's law at the sample's distance: y⁺ / (u_τ / ν) = the distance in cells.
                law = law.max((spalding_y_plus(speed / ut) / (ut / nu) - MATCHING_ROWS as f64).abs() / MATCHING_ROWS as f64);
                shear = shear.max((w.stress[0] * u[2] - w.stress[2] * u[0]).abs() / (s * speed));
                assert_eq!(w.stress[1], 0.0);
            }
            mass = mass.max((m / m0 - 1.0).abs());
            p = next;
        }
        println!(
            "{} matched wall, {} steps: momentum budget {budget:.1e}, carried vs sampled stress {carried:.1e}, Spalding's law at 3 cells {law:.1e}, direction {shear:.1e}, mass {mass:.1e}",
            collision.name(),
            setup.steps
        );
        assert!(budget < 1e-12 && carried < 1e-12 && law < 1e-10 && shear < 1e-12 && mass < 1e-13, "{budget} {carried} {law} {shear} {mass}");
    }
}

/// After one step, each node's rows below the sample relax with the mixing-length eddy viscosity
/// of Malaspinas and Sagaut's eq. 57 (μ_t/ρ = (κ y V)² |∂u/∂y|, κ 0.384, V = 1 − exp(−y⁺/26)),
/// with Spalding's ∂u/∂y at the node's friction velocity, and the rate τ̄ = 3 (ν + ν_t) + ½; the
/// sample row and the channel's centre keep the fluid's own rates.
#[test]
fn the_rows_below_relax_with_the_mixing_length() {
    let (setup, mut flow) = matched_channel(Collision::Regularized, 1.0);
    flow.step().unwrap();
    let nu = (setup.tau_plus - 0.5) / 3.0;
    let grid = *flow.grid();
    let nodes = flow.wall_nodes().unwrap().nodes().to_vec();
    let mut checked = 0;
    for w in &nodes {
        let (axis, inward) = (usize::from(w.axis), i32::from(w.inward));
        assert!(w.friction > 0.0, "the channel moves");
        for j in 1..MATCHING_ROWS {
            let cell = along(&flow, w.cell as usize, axis, inward, j);
            let y = j as f64;
            let y_plus = y * w.friction / nu;
            let (u_plus, slope) = spalding_u_plus(y_plus);
            assert!((spalding_y_plus(u_plus) - y_plus).abs() <= 1e-12 * y_plus);
            let du_dy = w.friction * w.friction / nu * slope;
            let l = MIXING_KAPPA * y * (1.0 - (-y_plus / MIXING_A_PLUS).exp());
            let nu_t = l * l * du_dy;
            let want = 1.0 / (3.0 * (nu + nu_t) + 0.5);
            let got = flow.layer_rate(cell).expect("a layer cell");
            assert!((got - want).abs() <= 1e-12 * want, "row {j}: {got} vs {want}");
            checked += 1;
        }
        assert!(flow.layer_rate(along(&flow, w.cell as usize, axis, inward, MATCHING_ROWS)).is_none());
    }
    assert_eq!(checked, nodes.len() * (MATCHING_ROWS - 1));
    let centre = grid.index([grid.cells[0] / 2, grid.cells[1] / 2, grid.cells[2] / 2]);
    assert!(flow.layer_rate(centre).is_none());
    // The other wall model keeps no layer.
    let (_, bounce) = channel_with(WallModel::SpaldingBounce, Collision::Regularized, 1.0);
    let mut bounce = bounce;
    bounce.step().unwrap();
    assert!(bounce.layer_rate(along(&bounce, nodes[0].cell as usize, usize::from(nodes[0].axis), i32::from(nodes[0].inward), 1)).is_none());
}

/// The populations after `steps` steps of the channel with `wall_model`, as one hash of their bits.
fn populations_hash(wall_model: WallModel, collision: Collision, steps: usize) -> u64 {
    let (setup, mut flow) = channel_with(wall_model, collision, steps as f64);
    for _ in 0..setup.steps {
        flow.step().unwrap();
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for cell in 0..flow.grid().len() {
        for q in 0..Q {
            h ^= flow.population(q, cell).to_bits();
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// `spalding_bounce` is not changed by the matched wall, checked in the same build: its populations
/// repeat bit for bit, its nodes sample the interior node they always did (one row in), it keeps no
/// layer rates, and the matched wall on the same channel takes a different path. A frozen hash is
/// not used: the initial field is built with the platform's sines and cosines, so its bits differ
/// between platforms.
#[test]
fn spalding_bounce_is_unchanged() {
    for collision in [Collision::Trt, Collision::Regularized, Collision::Hrr] {
        let plain = populations_hash(WallModel::SpaldingBounce, collision, 40);
        assert_eq!(plain, populations_hash(WallModel::SpaldingBounce, collision, 40), "{}", collision.name());
        assert_ne!(plain, populations_hash(WallModel::SpaldingBounceMatched, collision, 40), "{}", collision.name());
        let (_, flow) = channel_with(WallModel::SpaldingBounce, collision, 1.0);
        let nodes = flow.wall_nodes().unwrap();
        assert_eq!(nodes.rows(), 1);
        for w in nodes.nodes() {
            assert_eq!(w.sample, w.interior, "{}", collision.name());
        }
        assert!((0..flow.grid().len()).all(|c| flow.layer_rate(c).is_none()), "{}", collision.name());
    }
}

/// The matched wall refuses what it does not support, naming its own model, and the box needs
/// [`MATCHING_ROWS`] cells on each side of the sample row.
#[test]
fn the_matched_wall_refuses_what_it_does_not_support() {
    let dx = 0.125;
    let refused = |d: fieldcraft_model::FluidDomain, obstacles: &[fieldcraft_lbm::Obstacle], van_driest: Option<f64>| {
        let m = model(1.0, 0.01, d, bounced_with(WallModel::SpaldingBounceMatched, van_driest, 10.0 * dx, dx, Collision::Mrt));
        match Setup::new(&m, None, obstacles, &Limits::default()) {
            Ok(setup) => Flow::new(&setup, obstacles).unwrap_err().to_string(),
            Err(e) => e.to_string(),
        }
    };
    let periodic_channel = |cells_x: f64| {
        let mut d = domain([0.0; 3], [cells_x * dx, 1.0, 2.0 * dx], dx);
        for f in [&mut d.faces.x_min, &mut d.faces.x_max, &mut d.faces.z_min, &mut d.faces.z_max] {
            *f = PERIODIC;
        }
        d
    };
    let mut d = periodic_channel(8.0);
    d.faces.y_max = FaceCondition::MovingWall { velocity: [0.1, 0.0, 0.0] };
    let err = refused(d, &[], None);
    assert!(err.contains("not supported yet") && err.contains("spalding_bounce_matched") && err.contains("moving"), "{err}");
    let mut d = periodic_channel(8.0);
    d.obstacles = vec!["Block".into()];
    let block = obstacle("Block", cuboid([0.1, 0.4, 0.0], [0.3, 0.6, 0.25]), &["side"]);
    let err = refused(d, &[block], None);
    assert!(err.contains("not supported yet") && err.contains("spalding_bounce_matched") && err.contains("obstacle"), "{err}");
    let err = refused(periodic_channel(8.0), &[], Some(26.0));
    assert!(err.contains("not supported yet") && err.contains("spalding_bounce_matched") && err.contains("van_driest"), "{err}");
    let mut d = domain([0.0; 3], [6.0 * dx, 2.0 * dx, 2.0 * dx], dx);
    for f in [&mut d.faces.x_min, &mut d.faces.x_max, &mut d.faces.z_min, &mut d.faces.z_max] {
        *f = PERIODIC;
    }
    let err = refused(d, &[], None);
    assert!(
        err.contains("not supported yet") && err.contains("spalding_bounce_matched") && err.contains(&format!("at least {}", 2 * MATCHING_ROWS + 1)),
        "{err}"
    );
}

/// The same run on one thread and on several, bit for bit, with the matched wall.
#[test]
fn matched_results_are_bit_for_bit_the_same_on_one_and_many_threads() {
    let (n, re_tau, ub) = (6usize, 180.0, 15.7);
    let dx = 1.0 / n as f64;
    let mut d = domain([0.0; 3], [16.0, 2.0 + dx, 2.0], dx);
    for f in [&mut d.faces.x_min, &mut d.faces.x_max, &mut d.faces.z_min, &mut d.faces.z_max] {
        *f = PERIODIC;
    }
    d.acceleration = [1.0, 0.0, 0.0];
    d.initial_velocity = [ub, 0.0, 0.0];
    let dt = 0.1 * dx / ub;
    for collision in [Collision::Regularized, Collision::Hrr] {
        let m = model(1.0, 1.0 / re_tau, d.clone(), bounced_with(WallModel::SpaldingBounceMatched, None, 30.0 * dt, dt, collision));
        let solve =
            |threads: usize| fieldcraft_parallel::with_threads(threads, || fieldcraft_lbm::solve_fluid(&m, None, &[], &Limits::default()).unwrap());
        let one = solve(1);
        assert!(one.grid.len() > 3 * 4096, "several parallel blocks: {}", one.grid.len());
        assert_eq!(one, solve(7));
        assert_eq!(one, solve(3));
    }
}
