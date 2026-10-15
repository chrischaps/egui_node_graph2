//! Drawing cables, and the signal flowing through them.
//!
//! A cable shows its signal's recent past. The signal travels from the output
//! to the input at a steady [`FLOW_SPEED`], so the point a distance `s` along
//! a cable shows what the output was doing `s / FLOW_SPEED` seconds ago. A
//! note becomes a packet of light running down the wire, an envelope its
//! silhouette, and a slow LFO a wave riding the cable.
//!
//! Glyphs (chevrons, dots or comets) ride the flow at that same speed, so each
//! one carries the moment it left the output with it: bright through a note,
//! dark between notes. They always travel output to input, so the direction
//! of a patch can be read even in a still frame.

use egui::epaint::{Mesh, PathShape};
use egui::*;

/// How fast a signal travels along a cable, in graph units per second
pub const FLOW_SPEED: f32 = 170.0;

/// The mark that rides a cable's flow
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "persistence", derive(serde::Serialize, serde::Deserialize))]
pub enum FlowGlyph {
    /// Arrowheads pointing toward the input
    #[default]
    Chevron,
    /// Round beads
    Dot,
    /// Bright heads with fading tails
    Comet,
}

impl FlowGlyph {
    pub const ALL: [FlowGlyph; 3] = [FlowGlyph::Chevron, FlowGlyph::Dot, FlowGlyph::Comet];

    pub fn name(self) -> &'static str {
        match self {
            FlowGlyph::Chevron => "Chevrons",
            FlowGlyph::Dot => "Dots",
            FlowGlyph::Comet => "Comets",
        }
    }
}

/// What a cable knows of its signal: its level over the last few seconds.
///
/// Levels run 0 to 1 (or -1 to 1 for a bipolar signal); only their size sets
/// brightness. A *waveform* trace also draws its sign, as a line of light
/// that swings to one side of the cable or the other.
#[derive(Clone, Copy, Debug)]
pub struct SignalTrace<'a> {
    source: TraceSource<'a>,
    waveform: bool,
}

#[derive(Clone, Copy, Debug)]
enum TraceSource<'a> {
    Steady(f32),
    History {
        samples: &'a [f32],
        newest: usize,
        samples_per_second: f32,
        lag: f32,
    },
}

impl<'a> SignalTrace<'a> {
    /// A signal that has held `level` for as long as anyone can see
    pub fn steady(level: f32) -> Self {
        Self {
            source: TraceSource::Steady(level),
            waveform: false,
        }
    }

    /// A signal's history: `samples` is a ring buffer of levels taken
    /// `samples_per_second` apart, `newest` the index of the latest, which is
    /// `lag` seconds old. A NaN sample means there was no signal then.
    pub fn history(samples: &'a [f32], newest: usize, samples_per_second: f32, lag: f32) -> Self {
        Self {
            source: TraceSource::History {
                samples,
                newest,
                samples_per_second,
                lag,
            },
            waveform: false,
        }
    }

    /// Draws this signal's shape and sign, not just its strength
    pub fn as_waveform(mut self) -> Self {
        self.waveform = true;
        self
    }

    pub fn is_waveform(&self) -> bool {
        self.waveform
    }

    /// The signal as it was `age` seconds ago, or `None` if there was none.
    /// Past the end of the history, the oldest sample holds.
    pub fn at(&self, age: f32) -> Option<f32> {
        match self.source {
            TraceSource::Steady(level) => Some(level).filter(|level| level.is_finite()),
            TraceSource::History {
                samples,
                newest,
                samples_per_second,
                lag,
            } => {
                let len = samples.len();
                if len == 0 {
                    return None;
                }
                let back = (age - lag).max(0.0) * samples_per_second;
                let steps = back as usize;
                let frac = back - steps as f32;
                let sample = |steps: usize| {
                    let steps = steps.min(len - 1);
                    samples[(newest + len - steps) % len]
                };
                let (a, b) = (sample(steps), sample(steps + 1));
                let value = match (a.is_nan(), b.is_nan()) {
                    (false, false) => a + (b - a) * frac,
                    _ if frac < 0.5 => a,
                    _ => b,
                };
                Some(value).filter(|value| !value.is_nan())
            }
        }
    }
}

/// The look of a cable, in graph units unless noted
pub(crate) mod look {
    /// Width of a mono cable
    pub const CABLE_WIDTH: f32 = 5.0;
    /// Dark rim around every cable, so crossings and the grid stay distinct
    pub const OUTLINE: f32 = 1.2;
    pub const OUTLINE_ALPHA: u8 = 150;
    /// Brightness of an idle cable relative to its signal color, leaving
    /// headroom for the signal to light it
    pub const BASE_SHADE: f32 = 0.82;
    /// Core of light that runs along a live cable, as a fraction of its width
    pub const CORE_WIDTH: f32 = 0.5;
    /// How far toward white the core's color is lifted
    pub const CORE_LIFT: f32 = 0.3;
    /// Core brightness at full level
    pub const CORE_GAIN: f32 = 0.6;
    /// Soft glow, measured from the cable's center to where it fades out
    pub const GLOW_REACH: f32 = 6.5;
    pub const GLOW_GAIN: f32 = 0.2;
    /// How far a waveform trace swings from the cable's center at full scale
    pub const WAVE_SWING: f32 = 7.0;
    /// Over this length from each jack the swing eases in from nothing
    pub const WAVE_EASE: f32 = 26.0;
    /// Brightness of a waveform trace at zero, so a live control signal
    /// sitting at 0 still reads as present
    pub const WAVE_FLOOR: f32 = 0.15;
    /// Brightness follows level to this power, so quiet signals still show
    pub const LEVEL_GAMMA: f32 = 0.6;
    /// Space between glyphs along a cable
    pub const GLYPH_SPACING: f32 = 30.0;
    /// Glyphs fade in and out over this length at each jack
    pub const GLYPH_FADE: f32 = 12.0;
    /// How far toward white glyphs are lifted
    pub const GLYPH_LIFT: f32 = 0.6;
    /// Screen pixels between the points a cable is drawn through
    pub const SAMPLE_PX: f32 = 4.0;
    pub const MIN_SAMPLES: usize = 12;
    pub const MAX_SAMPLES: usize = 400;
    /// Soft edge, in screen pixels, of a cable showing through a node
    pub const GHOST_FEATHER: f32 = 1.0;
    /// How dark the other cables go while one is picked out, at full strength
    pub const SPOT_DIM: f32 = 0.55;
    /// How far toward white the picked-out cable is lifted
    pub const SPOT_LIFT: f32 = 0.3;
    /// The glow around a picked-out cable, from its edge to where it fades out
    pub const HALO_REACH: f32 = 9.0;
    pub const HALO_GAIN: f32 = 0.5;
    /// How strongly a picked-out cable shows through the nodes it passes under
    pub const SPOT_THROUGH: f32 = 0.55;
    /// Screen pixels off a cable's edge that the pointer still counts as on it
    pub const HOVER_REACH: f32 = 4.0;
}

/// Polyphonic cable look: one strand per channel, bundled side by side in a
/// dark sheath that pinches in where it plugs into a jack
pub(crate) mod poly_cable {
    /// Most strands drawn for one cable; channels past this aren't shown
    pub const MAX_STRANDS: usize = 16;
    /// How much each doubling of the channel count widens the bundle, as a
    /// fraction of a mono cable's width
    pub const WIDTH_PER_DOUBLING: f32 = 0.55;
    /// Fraction of the space between strand centers each strand fills; the
    /// rest shows the sheath, which is what separates the strands
    pub const STRAND_FILL: f32 = 0.72;
    /// Narrowest a strand gets on screen, so zoomed-out bundles keep their texture
    pub const MIN_STRAND_PX: f32 = 0.6;
    /// How far the sheath reaches past the outermost strands
    pub const SHEATH_PAD: f32 = 0.8;
    /// Sheath brightness relative to the cable color
    pub const SHEATH_SHADE: f32 = 0.32;
    /// Brightness of every other strand relative to the cable color
    pub const ALT_STRAND_SHADE: f32 = 0.8;
    /// Bundle width where it meets a jack (a jack is 10 across)
    pub const PLUG_WIDTH: f32 = 7.0;
    /// Length over which the bundle flares from the jack to its full width
    pub const FLARE_LENGTH: f32 = 28.0;
    /// Glyph size relative to the strand width
    pub const GLYPH_SIZE: f32 = 1.5;

    /// Bundle width of a cable carrying `channels` channels: it grows with
    /// each doubling rather than with every channel, so an 8-voice cable
    /// reads as heavy without swamping the patch
    pub fn bundle_width(mono_width: f32, channels: usize) -> f32 {
        mono_width * (1.0 + WIDTH_PER_DOUBLING * (channels as f32).log2())
    }
}

/// What flows through a cable this frame
pub(crate) struct CableFlow<'a, 'b> {
    /// Seconds since the app started, which sets where the glyphs are
    pub time: f64,
    pub glyph: FlowGlyph,
    /// One trace per channel; the cable is drawn as a bundle if more than one
    pub strands: &'b [Option<SignalTrace<'a>>],
}

/// Evaluate a cubic bezier curve at parameter t (0.0 to 1.0)
fn cubic_bezier_point(curve: &[Pos2; 4], t: f32) -> Pos2 {
    let [p0, p1, p2, p3] = *curve;
    let mt = 1.0 - t;
    let a = mt * mt * mt;
    let b = 3.0 * mt * mt * t;
    let c = 3.0 * mt * t * t;
    let d = t * t * t;
    pos2(
        a * p0.x + b * p1.x + c * p2.x + d * p3.x,
        a * p0.y + b * p1.y + c * p2.y + d * p3.y,
    )
}

/// Direction of travel along a cubic bezier curve at parameter t
fn cubic_bezier_tangent(curve: &[Pos2; 4], t: f32) -> Vec2 {
    let [p0, p1, p2, p3] = *curve;
    let mt = 1.0 - t;
    3.0 * mt * mt * (p1 - p0) + 6.0 * mt * t * (p2 - p1) + 3.0 * t * t * (p3 - p2)
}

/// Smooth 0-to-1 ramp of `x` across `0..edge`
fn smoothstep(edge: f32, x: f32) -> f32 {
    let x = (x / edge.max(f32::EPSILON)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Scale a color's brightness, keeping its alpha
pub(crate) fn shade(color: Color32, factor: f32) -> Color32 {
    let scale = |c: u8| (c as f32 * factor).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(scale(color.r()), scale(color.g()), scale(color.b()), color.a())
}

/// Move a color `amount` of the way toward white
fn lift(color: Color32, amount: f32) -> Color32 {
    let up = |c: u8| (c as f32 + (255.0 - c as f32) * amount).round() as u8;
    Color32::from_rgb(up(color.r()), up(color.g()), up(color.b()))
}

/// A cable's centerline, as points with their direction and their distance
/// along the cable, so the flow can be placed by distance rather than by
/// bezier parameter (which bunches up near the ends)
struct CablePath {
    points: Vec<Pos2>,
    /// Unit direction of travel at each point
    dirs: Vec<Vec2>,
    /// Distance from the output at each point, in screen pixels
    dist: Vec<f32>,
}

impl CablePath {
    fn new(curve: &[Pos2; 4]) -> Self {
        let rough = curve[0].distance(curve[1]) + curve[1].distance(curve[2]) + curve[2].distance(curve[3]);
        let segments = ((rough / look::SAMPLE_PX) as usize).clamp(look::MIN_SAMPLES, look::MAX_SAMPLES);
        let fallback = (curve[3] - curve[0]).normalized();
        let mut points = Vec::with_capacity(segments + 1);
        let mut dirs = Vec::with_capacity(segments + 1);
        let mut dist = Vec::with_capacity(segments + 1);
        let mut travelled = 0.0;
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let point = cubic_bezier_point(curve, t);
            if let Some(&prev) = points.last() {
                travelled += point.distance(prev);
            }
            let tangent = cubic_bezier_tangent(curve, t);
            let dir = if tangent.length_sq() > 1e-6 {
                tangent.normalized()
            } else {
                dirs.last().copied().unwrap_or(fallback)
            };
            points.push(point);
            dirs.push(dir);
            dist.push(travelled);
        }
        Self { points, dirs, dist }
    }

    fn length(&self) -> f32 {
        self.dist.last().copied().unwrap_or(0.0)
    }

    /// Point and direction at distance `s` from the output
    fn at(&self, s: f32) -> (Pos2, Vec2) {
        let i = self.dist.partition_point(|&d| d < s).clamp(1, self.points.len() - 1);
        let (d0, d1) = (self.dist[i - 1], self.dist[i]);
        let f = if d1 > d0 { ((s - d0) / (d1 - d0)).clamp(0.0, 1.0) } else { 0.0 };
        let point = self.points[i - 1].lerp(self.points[i], f);
        let dir = (self.dirs[i - 1] * (1.0 - f) + self.dirs[i] * f).normalized();
        (point, dir)
    }
}

/// Converts between distance along a cable and how long ago that part of
/// the signal left the output
struct Flow {
    /// Screen pixels the signal travels per second
    px_per_second: f32,
    zoom: f32,
    length: f32,
}

impl Flow {
    fn age(&self, s: f32) -> f32 {
        s / self.px_per_second
    }

    /// Brightness 0 to 1 of `trace` at distance `s` along the cable
    fn intensity(&self, trace: &SignalTrace, s: f32) -> f32 {
        match trace.at(self.age(s)) {
            None => 0.0,
            Some(value) => {
                let level = value.abs().min(1.0).powf(look::LEVEL_GAMMA);
                if trace.is_waveform() {
                    look::WAVE_FLOOR + (1.0 - look::WAVE_FLOOR) * level
                } else {
                    level
                }
            }
        }
    }

    /// Brightness 0 to 1 of `trace` at distance `s` from its size alone. A
    /// bundle's strands light this way, so idle voices stay dark even on a
    /// control cable.
    fn level(&self, trace: &SignalTrace, s: f32) -> f32 {
        trace
            .at(self.age(s))
            .map_or(0.0, |value| value.abs().min(1.0).powf(look::LEVEL_GAMMA))
    }

    /// How far a waveform trace sits from the cable's center at distance `s`
    fn swing(&self, trace: &SignalTrace, s: f32) -> f32 {
        if !trace.is_waveform() {
            return 0.0;
        }
        let ease = look::WAVE_EASE * self.zoom;
        let value = trace.at(self.age(s)).unwrap_or(0.0).clamp(-1.0, 1.0);
        value * look::WAVE_SWING * self.zoom * smoothstep(ease, s) * smoothstep(ease, self.length - s)
    }

    /// How visible a glyph is at distance `s`, fading at the jacks
    fn glyph_fade(&self, s: f32) -> f32 {
        let fade = look::GLYPH_FADE * self.zoom;
        smoothstep(fade, s) * smoothstep(fade, self.length - s)
    }
}

/// Adds a soft band of light along `centers` to `mesh`. Across the band,
/// `profile` lists (offset from the center, brightness) from one edge to the
/// other; along it, `color_at(i)` is the light at point `i`, an additive color.
fn add_light_band(
    mesh: &mut Mesh,
    centers: &[Pos2],
    normals: &[Vec2],
    profile: &[(f32, f32)],
    color_at: impl Fn(usize) -> Color32,
) {
    let across = profile.len() as u32;
    let first = mesh.vertices.len() as u32;
    for (i, (&center, &normal)) in centers.iter().zip(normals).enumerate() {
        let color = color_at(i);
        for &(offset, brightness) in profile {
            mesh.colored_vertex(center + normal * offset, color.linear_multiply(brightness));
        }
    }
    for i in 0..centers.len().saturating_sub(1) as u32 {
        let row = first + i * across;
        let next = row + across;
        for j in 0..across - 1 {
            mesh.add_triangle(row + j, row + j + 1, next + j);
            mesh.add_triangle(row + j + 1, next + j + 1, next + j);
        }
    }
}

/// Light a band would give at brightness `intensity`: additive, so where
/// light overlaps (a glow over a core, two cables crossing) it adds up
fn light(color: Color32, intensity: f32) -> Color32 {
    color.additive().linear_multiply(intensity.clamp(0.0, 1.0))
}

/// Draw one glyph at `pos`, pointing along `dir`. `size` is its length along
/// the cable; `alpha` how visible it is.
fn draw_glyph(out: &mut Vec<Shape>, glyph: FlowGlyph, pos: Pos2, dir: Vec2, size: f32, color: Color32, alpha: f32) {
    let normal = dir.rot90();
    let solid = color.gamma_multiply(alpha);
    match glyph {
        FlowGlyph::Chevron => {
            let half = size * 0.5;
            let tip = pos + dir * half;
            let back = pos - dir * half;
            let wings = [back + normal * size * 0.62, tip, back - normal * size * 0.62];
            out.push(Shape::Path(PathShape::line(
                wings.to_vec(),
                Stroke::new((size * 0.24).max(1.0), solid),
            )));
        }
        FlowGlyph::Dot => {
            out.push(Shape::circle_filled(pos, size * 0.36, solid));
        }
        FlowGlyph::Comet => {
            // A bright head trailing a wedge of fading light
            let head = pos + dir * size * 0.4;
            let tail = head - dir * size * 2.6;
            let width = size * 0.3;
            let mut mesh = Mesh::default();
            mesh.colored_vertex(head + normal * width, solid.gamma_multiply(0.85));
            mesh.colored_vertex(head - normal * width, solid.gamma_multiply(0.85));
            mesh.colored_vertex(tail, Color32::TRANSPARENT);
            mesh.add_triangle(0, 1, 2);
            out.push(Shape::mesh(mesh));
            out.push(Shape::circle_filled(head, width, solid));
        }
    }
}

/// Distances along a cable of `length` pixels where glyphs sit at `time`,
/// shifted by `stagger` of the spacing. They advance steadily with time
/// alone, so nothing about the signal can make them jump.
fn glyph_positions(time: f64, zoom: f32, length: f32, stagger: f32) -> impl Iterator<Item = f32> {
    let spacing = look::GLYPH_SPACING * zoom;
    let travelled = (time * FLOW_SPEED as f64 + (stagger * look::GLYPH_SPACING) as f64)
        .rem_euclid(look::GLYPH_SPACING as f64) as f32;
    let first = travelled * zoom;
    (0..)
        .map(move |k| first + k as f32 * spacing)
        .take_while(move |&s| s < length)
}

/// Draw a cable from an output to an input, with its signal flowing through
/// it, into `out`. Hands back its centerline, for the nodes it passes under
pub(crate) fn draw_connection(
    out: &mut Vec<Shape>,
    zoom: f32,
    src_pos: Pos2,
    dst_pos: Pos2,
    color: Color32,
    flow: &CableFlow,
) -> CableLine {
    let control_scale = ((dst_pos.x - src_pos.x) * zoom / 2.0).max(30.0 * zoom);
    let curve = [
        src_pos,
        src_pos + Vec2::X * control_scale,
        dst_pos - Vec2::X * control_scale,
        dst_pos,
    ];
    let path = CablePath::new(&curve);
    let motion = Flow {
        px_per_second: FLOW_SPEED * zoom,
        zoom,
        length: path.length(),
    };
    let (width, light) = if flow.strands.len() > 1 {
        let width = poly_cable::bundle_width(look::CABLE_WIDTH * zoom, flow.strands.len());
        (width, draw_bundle(out, zoom, color, &path, &motion, flow))
    } else {
        (look::CABLE_WIDTH * zoom, draw_mono(out, zoom, color, &path, &motion, flow))
    };
    CableLine::new(path, width, color, light)
}

/// A cable as the nodes it passes under show it: a faint band of its color
/// along just the stretch under each node, lit where its signal is, rather
/// than the whole cable again. A cable crossing a busy patch passes under
/// dozens of nodes, and this keeps each one to a few points.
pub(crate) struct CableLine {
    points: Vec<Pos2>,
    normals: Vec<Vec2>,
    /// Across the cable, or the whole bundle, in screen pixels
    width: f32,
    color: Color32,
    /// How lit the cable is at each point, 0 to 1; empty when it's dark
    light: Vec<f32>,
    bounds: Rect,
}

impl CableLine {
    fn new(path: CablePath, width: f32, color: Color32, light: Vec<f32>) -> Self {
        let bounds = Rect::from_points(&path.points).expand(width * 0.5 + look::GHOST_FEATHER);
        Self {
            normals: path.dirs.iter().map(|d| d.rot90()).collect(),
            points: path.points,
            width,
            color,
            light,
            bounds,
        }
    }

    /// How far `pos` is from the cable's edge, in screen pixels: 0 or less on it
    pub(crate) fn distance(&self, pos: Pos2) -> f32 {
        if !self.bounds.expand(look::HOVER_REACH).contains(pos) {
            return f32::INFINITY;
        }
        let nearest = self
            .points
            .windows(2)
            .map(|seg| {
                let (a, b) = (seg[0], seg[1]);
                let ab = b - a;
                let t = ((pos - a).dot(ab) / ab.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
                pos.distance(a + ab * t)
            })
            .fold(f32::INFINITY, f32::min);
        nearest - self.width * 0.5
    }

    /// A soft glow all along the cable, for the one picked out from the rest,
    /// at `strength` from 0 to 1. It's drawn under the cable
    pub(crate) fn halo(&self, zoom: f32, strength: f32) -> Shape {
        let mut mesh = Mesh::default();
        let half = self.width * 0.5;
        let reach = half + look::HALO_REACH * zoom;
        add_light_band(
            &mut mesh,
            &self.points,
            &self.normals,
            &[(-reach, 0.0), (-half, 1.0), (half, 1.0), (reach, 0.0)],
            |_| light(lift(self.color, look::SPOT_LIFT), strength * look::HALO_GAIN),
        );
        Shape::mesh(mesh)
    }

    /// Adds to `mesh` the stretches of this cable under `rect`, at `strength`
    /// of its brightness. Nothing is added where the cable only passes near.
    pub(crate) fn ghost(&self, mesh: &mut Mesh, rect: Rect, strength: f32) {
        if strength <= 0.0 || !rect.intersects(self.bounds) {
            return;
        }
        let reach = rect.expand(self.width * 0.5 + look::GHOST_FEATHER);
        let touches = |i: usize| Rect::from_two_pos(self.points[i], self.points[i + 1]).intersects(reach);
        let segments = self.points.len().saturating_sub(1);
        let mut i = 0;
        while i < segments {
            if !touches(i) {
                i += 1;
                continue;
            }
            let first = i;
            while i < segments && touches(i) {
                i += 1;
            }
            self.add_stretch(mesh, first, i, strength);
        }
    }

    /// The band from point `first` to `last`: the cable's body with its glow
    /// folded in, and a lifted core where it's lit, all faded to `strength`
    fn add_stretch(&self, mesh: &mut Mesh, first: usize, last: usize, strength: f32) {
        let centers = &self.points[first..=last];
        let normals = &self.normals[first..=last];
        let level = |i: usize| self.light.get(first + i).copied().unwrap_or(0.0);
        let half = self.width * 0.5;
        // The soft edge straddles the cable's edge, as a stroke's does, and a
        // cable thinner than it shows only as much as it covers
        let feather = look::GHOST_FEATHER * 0.5;
        let (inner, edge) = ((half - feather).max(0.0), half + feather);
        let strength = strength * (self.width / look::GHOST_FEATHER).min(1.0);
        let body = shade(self.color, look::BASE_SHADE);
        add_light_band(mesh, centers, normals, &[(-edge, 0.0), (-inner, 1.0), (inner, 1.0), (edge, 0.0)], |i| {
            brighten(body, light(self.color, level(i) * look::GLOW_GAIN)).gamma_multiply(strength)
        });
        if self.light.is_empty() {
            return;
        }
        let core = half * look::CORE_WIDTH;
        let core_color = lift(self.color, look::CORE_LIFT);
        add_light_band(mesh, centers, normals, &[(-core, 0.0), (0.0, 1.0), (core, 0.0)], |i| {
            light(core_color, level(i) * look::CORE_GAIN).gamma_multiply(strength)
        });
    }
}

/// `color` with additive `light` laid over it
fn brighten(color: Color32, light: Color32) -> Color32 {
    let [r, g, b, a] = color.to_array();
    let [lr, lg, lb, la] = light.to_array();
    Color32::from_rgba_premultiplied(
        r.saturating_add(lr),
        g.saturating_add(lg),
        b.saturating_add(lb),
        a.saturating_add(la),
    )
}

/// Draws a mono cable, handing back how lit it is along its length (empty
/// when it's dark)
fn draw_mono(out: &mut Vec<Shape>, zoom: f32, color: Color32, path: &CablePath, motion: &Flow, flow: &CableFlow) -> Vec<f32> {
    let width = look::CABLE_WIDTH * zoom;
    let rim = Color32::from_black_alpha(look::OUTLINE_ALPHA);
    out.push(Shape::Path(PathShape::line(
        path.points.clone(),
        Stroke::new(width + 2.0 * look::OUTLINE * zoom, rim),
    )));
    out.push(Shape::Path(PathShape::line(
        path.points.clone(),
        Stroke::new(width, shade(color, look::BASE_SHADE)),
    )));

    let Some(trace) = flow.strands.first().copied().flatten() else {
        return Vec::new();
    };

    let intensity: Vec<f32> = path.dist.iter().map(|&s| motion.intensity(&trace, s)).collect();
    if intensity.iter().all(|&i| i <= 0.0) {
        return Vec::new();
    }
    // A waveform's light rides beside the cable, at its value
    let normals: Vec<Vec2> = path.dirs.iter().map(|d| d.rot90()).collect();
    let centers: Vec<Pos2> = path
        .points
        .iter()
        .zip(&normals)
        .zip(&path.dist)
        .map(|((&p, &n), &s)| p + n * motion.swing(&trace, s))
        .collect();

    let mut mesh = Mesh::default();
    let reach = look::GLOW_REACH * zoom;
    add_light_band(
        &mut mesh,
        &centers,
        &normals,
        &[(-reach, 0.0), (-reach * 0.35, 0.45), (0.0, 1.0), (reach * 0.35, 0.45), (reach, 0.0)],
        |i| light(color, intensity[i] * look::GLOW_GAIN),
    );
    let core = width * look::CORE_WIDTH * 0.5;
    let core_color = lift(color, look::CORE_LIFT);
    add_light_band(
        &mut mesh,
        &centers,
        &normals,
        &[(-core, 0.0), (-core * 0.4, 1.0), (core * 0.4, 1.0), (core, 0.0)],
        |i| light(core_color, intensity[i] * look::CORE_GAIN),
    );
    out.push(Shape::mesh(mesh));

    // Glyphs ride the light; on a waveform they follow its swing
    let glyph_color = lift(color, look::GLYPH_LIFT);
    let size = width * 1.5;
    let at = |s: f32| {
        let (p, d) = path.at(s);
        p + d.rot90() * motion.swing(&trace, s)
    };
    for s in glyph_positions(flow.time, zoom, motion.length, 0.0) {
        let alpha = motion.intensity(&trace, s) * motion.glyph_fade(s);
        if alpha < 0.02 {
            continue;
        }
        let nudge = 1.5 * zoom;
        let dir = (at(s + nudge) - at(s - nudge)).normalized();
        draw_glyph(out, flow.glyph, at(s), dir, size, glyph_color, alpha);
    }
    intensity
}

/// A polyphonic cable: each strand runs at a fixed offset from the curve,
/// squeezed together near the ends so the bundle fits into the jacks, and
/// lights with its own voice. Hands back how lit the bundle is along its
/// length (empty when it's dark)
fn draw_bundle(out: &mut Vec<Shape>, zoom: f32, color: Color32, path: &CablePath, motion: &Flow, flow: &CableFlow) -> Vec<f32> {
    let channels = flow.strands.len();
    let bundle_width = poly_cable::bundle_width(look::CABLE_WIDTH * zoom, channels);
    let spacing = bundle_width / channels as f32;
    let strand_width = (spacing * poly_cable::STRAND_FILL).max(poly_cable::MIN_STRAND_PX);
    let length = motion.length.max(1.0);
    let pinch = (poly_cable::PLUG_WIDTH * zoom / bundle_width).min(1.0);
    let flare = (poly_cable::FLARE_LENGTH * zoom / length).min(0.4);
    let spread = |s: f32| {
        let t = s / length;
        pinch + (1.0 - pinch) * smoothstep(flare, t) * smoothstep(flare, 1.0 - t)
    };
    let offset = |channel: usize| (channel as f32 - (channels - 1) as f32 / 2.0) * spacing;
    let normals: Vec<Vec2> = path.dirs.iter().map(|d| d.rot90()).collect();
    let strand_paths: Vec<Vec<Pos2>> = (0..channels)
        .map(|channel| {
            path.points
                .iter()
                .zip(&normals)
                .zip(&path.dist)
                .map(|((&p, &n), &s)| p + n * offset(channel) * spread(s))
                .collect()
        })
        .collect();

    // The sheath: each strand's path drawn wide enough to meet its
    // neighbours, so together they make one dark ribbon, edged in a rim
    let rim = Color32::from_black_alpha(look::OUTLINE_ALPHA);
    let sheath_width = spacing.max(strand_width) + 2.0 * poly_cable::SHEATH_PAD * zoom;
    for strand in [strand_paths.first(), strand_paths.last()].into_iter().flatten() {
        out.push(Shape::Path(PathShape::line(
            strand.clone(),
            Stroke::new(sheath_width + 2.0 * look::OUTLINE * zoom, rim),
        )));
    }
    let sheath_color = shade(color, poly_cable::SHEATH_SHADE);
    for strand in &strand_paths {
        out.push(Shape::Path(PathShape::line(strand.clone(), Stroke::new(sheath_width, sheath_color))));
    }

    // The strands, alternating in tone like the cores of a ribbon cable
    let base = shade(color, look::BASE_SHADE);
    for (channel, strand) in strand_paths.iter().enumerate() {
        let strand_color = if channel % 2 == 0 {
            base
        } else {
            shade(base, poly_cable::ALT_STRAND_SHADE)
        };
        out.push(Shape::Path(PathShape::line(strand.clone(), Stroke::new(strand_width, strand_color))));
    }

    // Each strand's light, and a glow around the bundle that grows with how
    // much of it is lit, so a chord glows brighter than a single note
    let intensity: Vec<Vec<f32>> = flow
        .strands
        .iter()
        .map(|trace| match trace {
            Some(trace) => path.dist.iter().map(|&s| motion.level(trace, s)).collect(),
            None => vec![0.0; path.dist.len()],
        })
        .collect();
    let fullness: Vec<f32> = (0..path.dist.len())
        .map(|i| bundle_fullness(intensity.iter().map(|strand| strand[i])))
        .collect();
    if fullness.iter().all(|&i| i <= 0.0) {
        return Vec::new();
    }
    let mut mesh = Mesh::default();
    let half = bundle_width * 0.5;
    let reach = half + look::GLOW_REACH * zoom;
    add_light_band(
        &mut mesh,
        &path.points,
        &normals,
        &[(-reach, 0.0), (-half, 0.4), (half, 0.4), (reach, 0.0)],
        |i| light(color, fullness[i] * look::GLOW_GAIN),
    );
    let core_color = lift(color, look::CORE_LIFT);
    let core = strand_width * 0.5;
    for (strand, levels) in strand_paths.iter().zip(&intensity) {
        add_light_band(
            &mut mesh,
            strand,
            &normals,
            &[(-core, 0.0), (0.0, 1.0), (core, 0.0)],
            |i| light(core_color, levels[i] * look::CORE_GAIN),
        );
    }
    out.push(Shape::mesh(mesh));

    let glyph_color = lift(color, look::GLYPH_LIFT);
    let size = strand_width * poly_cable::GLYPH_SIZE * 2.0;
    for (channel, trace) in flow.strands.iter().enumerate() {
        let Some(trace) = trace else { continue };
        // Stagger by the golden ratio so the strands' glyphs never line up
        let stagger = (channel as f32 * 0.618_034).fract();
        for s in glyph_positions(flow.time, zoom, motion.length, stagger) {
            let alpha = motion.level(trace, s) * motion.glyph_fade(s);
            if alpha < 0.02 {
                continue;
            }
            let (p, dir) = path.at(s);
            let pos = p + dir.rot90() * offset(channel) * spread(s);
            draw_glyph(out, flow.glyph, pos, dir, size, glyph_color, alpha);
        }
    }
    fullness
}

/// How lit a bundle is, 0 to 1, from its strands' brightness: the square
/// root of their mean, so one voice of eight glows about a third as bright
/// as all eight, and each added voice still shows
fn bundle_fullness(levels: impl ExactSizeIterator<Item = f32>) -> f32 {
    let count = levels.len().max(1) as f32;
    (levels.sum::<f32>() / count).max(0.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_trace_holds_its_level() {
        let trace = SignalTrace::steady(0.4);
        assert_eq!(trace.at(0.0), Some(0.4));
        assert_eq!(trace.at(100.0), Some(0.4));
    }

    #[test]
    fn history_reads_back_in_time() {
        // Ring of 4 at 10 samples per second; newest is index 1
        let samples = [0.2, 0.3, f32::NAN, 0.1];
        let trace = SignalTrace::history(&samples, 1, 10.0, 0.0);
        let near = |age: f32, want: f32| (trace.at(age).unwrap() - want).abs() < 1e-4;
        assert!(near(0.0, 0.3));
        assert!(near(0.1, 0.2));
        assert!(near(0.05, 0.25));
        assert!(near(0.2, 0.1));
        // Before the signal began there was none
        assert_eq!(trace.at(0.3), None);
        assert_eq!(trace.at(5.0), None);
    }

    #[test]
    fn history_holds_its_oldest_sample() {
        let samples = [0.5, 0.7];
        let trace = SignalTrace::history(&samples, 1, 10.0, 0.0);
        assert!((trace.at(5.0).unwrap() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn history_accounts_for_lag() {
        let samples = [0.0, 1.0];
        let trace = SignalTrace::history(&samples, 1, 10.0, 0.05);
        assert_eq!(trace.at(0.0), Some(1.0));
        assert_eq!(trace.at(0.05), Some(1.0));
        assert!((trace.at(0.1).unwrap() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn a_chord_fills_the_bundle_more_than_a_note() {
        let one = bundle_fullness([1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0].into_iter());
        let four = bundle_fullness([1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0].into_iter());
        let all = bundle_fullness([1.0; 8].into_iter());
        assert!((one - 0.354).abs() < 0.01);
        assert!(one < four && four < all);
        assert!((all - 1.0).abs() < 1e-6);
    }

    #[test]
    fn glyphs_advance_steadily() {
        let at = |time| glyph_positions(time, 1.0, 1000.0, 0.0).next().unwrap();
        let step = at(0.01) - at(0.0);
        assert!((step - FLOW_SPEED * 0.01).abs() < 1e-3);
        assert!(glyph_positions(0.0, 1.0, 100.0, 0.0).all(|s| s < 100.0));
    }

    #[test]
    fn path_is_measured_by_distance() {
        let curve = [pos2(0.0, 0.0), pos2(30.0, 0.0), pos2(70.0, 0.0), pos2(100.0, 0.0)];
        let path = CablePath::new(&curve);
        assert!((path.length() - 100.0).abs() < 0.5);
        let (mid, dir) = path.at(50.0);
        assert!((mid.x - 50.0).abs() < 0.5);
        assert!((dir.x - 1.0).abs() < 1e-3);
    }

    fn lit_line(src: Pos2, dst: Pos2) -> CableLine {
        let flow = CableFlow { time: 0.0, glyph: FlowGlyph::Dot, strands: &[Some(SignalTrace::steady(1.0))] };
        draw_connection(&mut Vec::new(), 1.0, src, dst, Color32::from_rgb(80, 160, 240), &flow)
    }

    fn ghost_of(line: &CableLine, rect: Rect, strength: f32) -> Mesh {
        let mut mesh = Mesh::default();
        line.ghost(&mut mesh, rect, strength);
        mesh
    }

    #[test]
    fn the_pointer_is_on_a_cable_along_its_curve_not_its_bounds() {
        let line = lit_line(pos2(0.0, 0.0), pos2(400.0, 0.0));
        assert!(line.distance(pos2(200.0, 1.0)) <= 0.0);
        assert!(line.distance(pos2(200.0, 2.5 + look::HOVER_REACH - 0.5)) <= look::HOVER_REACH);
        assert!(line.distance(pos2(200.0, 20.0)) > look::HOVER_REACH);
        // Inside a diagonal's bounding box, but nowhere near the cable
        let diagonal = lit_line(pos2(0.0, 0.0), pos2(600.0, 400.0));
        assert!(diagonal.distance(pos2(560.0, 40.0)) > look::HOVER_REACH);
    }

    #[test]
    fn a_node_shows_only_the_stretch_of_cable_under_it() {
        let line = lit_line(pos2(0.0, 0.0), pos2(400.0, 0.0));
        let node = Rect::from_min_max(pos2(150.0, -40.0), pos2(250.0, 40.0));
        let mesh = ghost_of(&line, node, 0.15);
        assert!(!mesh.is_empty());
        // A sample past each side of the node at most; the clip trims the rest
        assert!(mesh.vertices.iter().all(|v| v.pos.x > 140.0 && v.pos.x < 260.0));
    }

    #[test]
    fn a_node_inside_a_cables_bounds_but_off_its_path_shows_nothing() {
        // A long diagonal: its bounding box covers corners it never visits
        let line = lit_line(pos2(0.0, 0.0), pos2(600.0, 400.0));
        let corner = Rect::from_min_max(pos2(450.0, 20.0), pos2(580.0, 120.0));
        assert!(corner.intersects(line.bounds));
        assert!(ghost_of(&line, corner, 0.15).is_empty());
    }

    #[test]
    fn a_ghost_keeps_the_cables_hue_at_a_fraction_of_its_strength() {
        let line = lit_line(pos2(0.0, 0.0), pos2(400.0, 0.0));
        let node = Rect::from_min_max(pos2(150.0, -40.0), pos2(250.0, 40.0));
        let brightest = |strength| ghost_of(&line, node, strength).vertices.iter().map(|v| v.color.b()).max();
        let (faint, full) = (brightest(0.15).unwrap(), brightest(1.0).unwrap());
        assert!(faint > 0 && faint < full / 4, "{faint} of {full}");
        assert!(ghost_of(&line, node, 0.0).is_empty());
        // A blue cable shows through blue
        assert!(ghost_of(&line, node, 0.15).vertices.iter().all(|v| v.color.b() >= v.color.r()));
    }
}
