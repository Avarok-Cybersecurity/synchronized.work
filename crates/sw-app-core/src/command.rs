//! The canvas commands the App hands the host, shaped exactly like the SDK's `DrawCommand`
//! (`calls/canvas.rs`: the five ABI-1 commands, plus the five CW-5 commands of the frozen
//! `canvas-ext-abi` contract §1). The binding maps each variant 1:1 onto the SDK type; the
//! limits below are the contract's, checked here before anything is emitted.

/// Colours are `0xRRGGBBAA`; coordinates are CSS pixels.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    Clear {
        color: u32,
    },
    FillRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: u32,
    },
    StrokeRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        line_width: f32,
        color: u32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        line_width: f32,
        color: u32,
    },
    FillText {
        x: f32,
        y: f32,
        text: String,
        px: f32,
        color: u32,
    },
    /// CW-5.
    FillCircle {
        cx: f32,
        cy: f32,
        r: f32,
        color: u32,
    },
    /// CW-5.
    StrokeCircle {
        cx: f32,
        cy: f32,
        r: f32,
        line_width: f32,
        color: u32,
    },
    /// CW-5. `PATH_POINTS` points; `stroke` is `(line_width, color)`; never both `None`.
    Path {
        points: Vec<(f32, f32)>,
        closed: bool,
        fill: Option<u32>,
        stroke: Option<(f32, u32)>,
    },
    /// CW-5. Radians, clockwise from +x.
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        start: f32,
        end: f32,
        line_width: f32,
        color: u32,
    },
    /// CW-5. Paints nothing: the host's focusable, labelled hit region.
    Region {
        id: String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: String,
        role: RegionRole,
    },
}

/// `RegionRole` of the contract (`button`, `link`, `img`, `group` on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionRole {
    Button,
    Link,
    Img,
    Group,
}

impl From<sw_graph::RegionRole> for RegionRole {
    fn from(r: sw_graph::RegionRole) -> Self {
        match r {
            sw_graph::RegionRole::Button => Self::Button,
            sw_graph::RegionRole::Link => Self::Link,
            sw_graph::RegionRole::Img => Self::Img,
            sw_graph::RegionRole::Group => Self::Group,
        }
    }
}

/// The contract's limits (canvas-ext-abi §1): a frame breaking any of them is refused whole
/// by the host, so the painters refuse it first.
pub mod limits {
    /// `Path.points.len()` is in this range.
    pub const PATH_POINTS: core::ops::RangeInclusive<usize> = 2..=256;
    /// `Region.id` length in bytes, each one of `[a-z0-9_-]`.
    pub const REGION_ID: core::ops::RangeInclusive<usize> = 1..=64;
    /// `Region.label`, in characters.
    pub const REGION_LABEL_CHARS: usize = 256;
    /// `Region`s in one frame.
    pub const REGIONS_PER_FRAME: usize = 512;
}

/// Whether `id` is in the contract's region-id grammar `[a-z0-9_-]{1,64}`.
pub fn is_region_id(id: &str) -> bool {
    limits::REGION_ID.contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}
