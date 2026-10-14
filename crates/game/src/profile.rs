//! Frame-time breakdown (`AC1_PERF=1`): what a frame costs, measured on the wall clock (so `AC1_STEP` runs, every frame
//! the same game time, still show the real frame rate).
//!
//! Every `AC1_PERF_EVERY` seconds (default 5) a `perf:` line: the frames a second, the frame time's mean, median, 95th and
//! 99th percentile and worst (ms), the GPU's time a frame by pass (Bevy's render diagnostics: every pass that records a
//! span; the shadow maps record none, so their cost shows only as a difference with shadows off), and, in a build with
//! the `profile` feature (`cargo build --release -p ac1 --features profile`: Bevy's `trace` spans on every system), the
//! systems that took the most CPU, in ms a frame, both worlds (main and render) and all threads summed. At exit a
//! `perf: run` line sums the run after its first `AC1_PERF_SKIP` seconds (default 8: the city streaming in).

use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

pub struct ProfilePlugin;

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("AC1_PERF").is_err() {
            return;
        }
        app.init_resource::<Frames>().add_systems(First, tick).add_systems(Last, (report, summary).chain());
    }
}

fn env_f32(k: &str, d: f32) -> f32 {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

#[derive(Resource, Default)]
struct Frames {
    last: Option<Instant>,
    start: Option<Instant>,
    /// This window's frame times (ms), and the run's after the warm-up.
    window: Vec<f32>,
    run: Vec<f32>,
    window_start: Option<Instant>,
    /// GPU ms a frame by pass, summed over the windows' reports since the warm-up (and how many).
    gpu_run: HashMap<String, (f64, u32)>,
    cpu_run: HashMap<String, f64>,
    run_frames: u64,
}

fn tick(mut f: ResMut<Frames>) {
    let now = Instant::now();
    let start = *f.start.get_or_insert(now);
    if let Some(prev) = f.last.replace(now) {
        let ms = (now - prev).as_secs_f32() * 1000.0;
        f.window.push(ms);
        if (now - start).as_secs_f32() > env_f32("AC1_PERF_SKIP", 8.0) {
            f.run.push(ms);
        }
    }
    f.window_start.get_or_insert(now);
}

/// (mean, p50, p95, p99, max) of frame times.
fn stats(v: &[f32]) -> Option<(f32, f32, f32, f32, f32)> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f32::total_cmp);
    let at = |q: f32| s[((s.len() - 1) as f32 * q).round() as usize];
    Some((s.iter().sum::<f32>() / s.len() as f32, at(0.5), at(0.95), at(0.99), s[s.len() - 1]))
}

/// The GPU passes' times (ms a frame, averaged by Bevy), the deepest level of each path.
fn gpu_passes(diag: &bevy::diagnostic::DiagnosticsStore) -> Vec<(String, f64)> {
    let mut v: Vec<(String, f64)> = diag
        .iter()
        .filter(|d| d.path().as_str().ends_with("/elapsed_gpu"))
        .filter_map(|d| Some((d.path().as_str().trim_start_matches("render/").trim_end_matches("/elapsed_gpu").to_string(), d.average()?)))
        .collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
}

/// The meshes the report counts: seen, skinned, not casting.
type MeshQuery<'w, 's> = Query<'w, 's, (&'static ViewVisibility, Has<bevy::mesh::skinning::SkinnedMesh>, Has<bevy::light::NotShadowCaster>), With<Mesh3d>>;

fn report(mut f: ResMut<Frames>, diag: Res<bevy::diagnostic::DiagnosticsStore>, meshes: MeshQuery, entities: Query<()>) {
    let now = Instant::now();
    let Some(ws) = f.window_start else { return };
    let secs = (now - ws).as_secs_f32();
    if secs < env_f32("AC1_PERF_EVERY", 5.0) {
        return;
    }
    let warm = f.start.is_some_and(|s| (now - s).as_secs_f32() > env_f32("AC1_PERF_SKIP", 8.0));
    let frames = f.window.len();
    if let Some((mean, p50, p95, p99, max)) = stats(&f.window) {
        let gpu = gpu_passes(&diag);
        // (The top-level passes: the GPU's frame as Bevy times it.)
        let top: f64 = gpu.iter().filter(|(p, _)| !p.contains('/')).map(|(_, ms)| ms).sum();
        let passes: Vec<String> = gpu.iter().take(8).map(|(p, ms)| format!("{p} {ms:.2}")).collect();
        info!(
            "perf: {:.1} fps, frame {mean:.2} ms (p50 {p50:.2}, p95 {p95:.2}, p99 {p99:.2}, max {max:.1}); GPU {top:.2} ms: {}",
            frames as f32 / secs,
            passes.join(", ")
        );
        // (What there is to draw: meshes, those seen by a view (the camera or a shadow cascade), skinned, casting.)
        let (mut n, mut seen, mut skinned, mut casters) = (0, 0, 0, 0);
        for (v, skin, no_shadow) in &meshes {
            n += 1;
            seen += v.get() as usize;
            skinned += skin as usize;
            casters += (!no_shadow) as usize;
        }
        info!("perf: {} entities, {n} meshes, {seen} seen, {skinned} skinned, {casters} casting shadows", entities.iter().len());
        if warm {
            for (p, ms) in gpu {
                let e = f.gpu_run.entry(p).or_default();
                e.0 += ms;
                e.1 += 1;
            }
        }
    }
    let cpu = take_cpu();
    if !cpu.is_empty() {
        let mut v: Vec<(String, f64)> = cpu.into_iter().collect();
        if warm {
            for (k, ms) in &v {
                *f.cpu_run.entry(k.clone()).or_default() += ms;
            }
            f.run_frames += frames as u64;
        }
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        let total: f64 = v.iter().map(|x| x.1).sum::<f64>() / frames.max(1) as f64;
        let top: Vec<String> = v.iter().take(14).map(|(k, ms)| format!("{} {:.2}", short(k), ms / frames.max(1) as f64)).collect();
        info!("perf: CPU {total:.2} ms a frame in systems: {}", top.join(", "));
    }
    f.window.clear();
    f.window_start = Some(now);
}

fn summary(f: Res<Frames>, mut exit: MessageReader<AppExit>) {
    if exit.read().next().is_none() {
        return;
    }
    let Some((mean, p50, p95, p99, max)) = stats(&f.run) else { return };
    let secs: f32 = f.run.iter().sum::<f32>() / 1000.0;
    let mut gpu: Vec<(&String, f64)> = f.gpu_run.iter().map(|(k, (s, n))| (k, s / *n as f64)).collect();
    gpu.sort_by(|a, b| b.1.total_cmp(&a.1));
    let top: f64 = gpu.iter().filter(|(p, _)| !p.contains('/')).map(|(_, ms)| ms).sum();
    let passes: Vec<String> = gpu.iter().take(10).map(|(p, ms)| format!("{p} {ms:.2}")).collect();
    info!(
        "perf: run {} frames in {secs:.1} s, {:.1} fps, frame {mean:.2} ms (p50 {p50:.2}, p95 {p95:.2}, p99 {p99:.2}, max {max:.1}); GPU {top:.2} ms: {}",
        f.run.len(),
        f.run.len() as f32 / secs.max(1e-3),
        passes.join(", ")
    );
    if f.run_frames > 0 {
        let mut v: Vec<(&String, f64)> = f.cpu_run.iter().map(|(k, ms)| (k, ms / f.run_frames as f64)).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        let total: f64 = v.iter().map(|x| x.1).sum();
        let top: Vec<String> = v.iter().take(25).map(|(k, ms)| format!("{} {ms:.3}", short(k))).collect();
        info!("perf: run CPU {total:.2} ms a frame in systems: {}", top.join(", "));
    }
}

/// A system's name without its crate paths (`ac1::character::animate` -> `character::animate`).
fn short(name: &str) -> String {
    let base = name.split('<').next().unwrap_or(name);
    let parts: Vec<&str> = base.split("::").collect();
    let keep = if parts.first() == Some(&"ac1") { &parts[1..] } else { &parts[parts.len().saturating_sub(2)..] };
    keep.join("::")
}

// --- The systems' CPU time (the `profile` feature: Bevy's `trace` spans) ------------------------------------------

/// CPU ms by span name since the last report.
static CPU: Mutex<Option<HashMap<String, f64>>> = Mutex::new(None);

fn take_cpu() -> HashMap<String, f64> {
    CPU.lock().map(|mut m| m.take().unwrap_or_default()).unwrap_or_default()
}

/// The tracing layer that times Bevy's `system` and `system_commands` spans (`LogPlugin::custom_layer`).
#[cfg(feature = "profile")]
pub fn layer(_app: &mut App) -> Option<bevy::log::BoxedLayer> {
    std::env::var("AC1_PERF").is_ok().then(|| Box::new(SpanTimes) as bevy::log::BoxedLayer)
}

#[cfg(feature = "profile")]
struct SpanTimes;

#[cfg(feature = "profile")]
struct Timed {
    name: String,
    entered: Option<Instant>,
}

#[cfg(feature = "profile")]
struct NameField(Option<String>);

#[cfg(feature = "profile")]
impl bevy::log::tracing::field::Visit for NameField {
    fn record_str(&mut self, field: &bevy::log::tracing::field::Field, value: &str) {
        if field.name() == "name" {
            self.0 = Some(value.to_string());
        }
    }
    fn record_debug(&mut self, field: &bevy::log::tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "name" {
            self.0 = Some(format!("{value:?}").trim_matches('"').to_string());
        }
    }
}

#[cfg(feature = "profile")]
impl<S> bevy::log::tracing_subscriber::Layer<S> for SpanTimes
where
    S: bevy::log::tracing::Subscriber + for<'a> bevy::log::tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &bevy::log::tracing::span::Attributes<'_>,
        id: &bevy::log::tracing::span::Id,
        ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
    ) {
        let kind = attrs.metadata().name();
        if let Some(n) = kind.strip_prefix("ac1:") {
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(Timed { name: format!("~{n}"), entered: None });
            }
            return;
        }
        if kind != "system" && kind != "system_commands" {
            return;
        }
        let mut v = NameField(None);
        attrs.record(&mut v);
        let Some(name) = v.0 else { return };
        let name = if kind == "system_commands" { format!("{name} (commands)") } else { name };
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Timed { name, entered: None });
        }
    }
    fn on_enter(&self, id: &bevy::log::tracing::span::Id, ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>) {
        if let Some(span) = ctx.span(id)
            && let Some(t) = span.extensions_mut().get_mut::<Timed>()
        {
            t.entered = Some(Instant::now());
        }
    }
    fn on_exit(&self, id: &bevy::log::tracing::span::Id, ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let mut ext = span.extensions_mut();
        let Some(t) = ext.get_mut::<Timed>() else { return };
        let Some(at) = t.entered.take() else { return };
        let ms = at.elapsed().as_secs_f64() * 1000.0;
        if let Ok(mut m) = CPU.lock() {
            *m.get_or_insert_with(HashMap::new).entry(t.name.clone()).or_default() += ms;
        }
    }
}
