//! Source: original
//! The command line as clap reads it (docs/decisions/2026-11-13-clap.md): each command, what it takes, and the help an
//! agent reads first. Ids are `kind:slot/generation` (S6) and kinds are the names the reports give.

use std::path::PathBuf;

use clap::builder::PossibleValuesParser;
use clap::{Args, Parser, Subcommand};
use occam::{EntityKind, EntityRef};

/// The kinds a model holds, which `list` takes.
pub const LISTED: [EntityKind; 11] = [
    EntityKind::Body,
    EntityKind::Shell,
    EntityKind::Face,
    EntityKind::Loop,
    EntityKind::Wire,
    EntityKind::EdgeUse,
    EntityKind::Edge,
    EntityKind::Vertex,
    EntityKind::Surface,
    EntityKind::Curve,
    EntityKind::ParamCurve,
];

/// What an id is written as, said when one is not.
pub const ID_FORM: &str = "an id as kind:slot/generation, such as edge:41/1";

/// The occam CAD kernel, for agents and people.
///
/// Each command gives its result in words, or with --json in one JSON form on standard output. Every command that takes
/// ids answers each one apart: an id that names nothing is refused alone, and the others are answered. The exit status is
/// 0 when everything asked was answered, 1 when anything was refused, and 2 when the command line is wrong. Lengths are
/// in millimetres and angles in radians.
///
/// A model is built by `occam run`; `occam run --help` shows how to write the request it runs.
#[derive(Debug, Parser)]
#[command(name = "occam", version, max_term_width = 100)]
pub struct Cli {
    /// Give the result, or the reports refusing it, in one JSON form on standard output.
    #[arg(long, global = true)]
    pub json: bool,
    /// The command.
    #[command(subcommand)]
    pub command: Command,
}

/// What occam can be asked to do.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run a request on a model, and write the model back if the whole request went through.
    ///
    /// --help shows how to write a request.
    #[command(after_long_help = occam::guide())]
    Run(RunArgs),
    /// List every live entity of each kind given, in slot order.
    List(ListArgs),
    /// List each entity's neighbours.
    Adjacent(IdsArgs),
    /// Describe each entity: its form, its values, the step that made it and its neighbours.
    Describe(IdsArgs),
    /// Check the whole model.
    Check(ModelArgs),
    /// Mesh the model for display, write its buffers, and say how each face meshed.
    ///
    /// The buffers are written beside the model as <model>.mesh, or to --out: the counts of vertices, triangles and
    /// faces as three u64, then the positions, the unit normals and each vertex's (u, v) on its face as f64, then the
    /// indices, counted from the start of their face's vertex block, and a table of six u32 per face (its slot and
    /// generation, its first vertex and how many, its first triangle and how many), all little-endian. A face or edge
    /// refused gives a report; its use-deflection remedy names a --deflection at which it meshes.
    Mesh(MeshArgs),
    /// The distance between two points, to a tolerance.
    Distance(DistanceArgs),
    /// Show the model live in a page on this machine, which redraws it each time a run saves it, and return.
    ///
    /// Starts the model's viewer if none runs, a server of its own that watches the model file, prints the page's
    /// address and opens it in a browser, the one $BROWSER names or the system's. A second `occam view` of the same
    /// model prints the same address and starts nothing. --json gives the address, its token and the model's digest.
    /// A file that is not a model, or is not there, is reported here and in the page, and the viewer keeps watching.
    /// --status says whether the viewer runs and where, and --stop stops it; it also stops itself once no page has
    /// been open for an hour. The page needs a browser with WebGPU.
    #[cfg(not(target_family = "wasm"))]
    View(ViewArgs),
}

/// A model to read.
#[derive(Debug, Args)]
pub struct ModelArgs {
    /// The model: a native file, readable JSON or its binary twin.
    #[arg(long, value_name = "FILE")]
    pub model: PathBuf,
}

/// What `run` takes.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// The model: read if it exists and made new if not, and written back only if the whole request goes through.
    #[arg(long, value_name = "FILE")]
    pub model: PathBuf,
    /// The request: a JSON file of steps.
    #[arg(value_name = "REQUEST")]
    pub request: PathBuf,
    /// Write the model as the binary twin. Otherwise a model keeps its encoding, and a new one is readable JSON.
    #[arg(long)]
    pub binary: bool,
}

/// What `list` takes.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// The model.
    #[command(flatten)]
    pub model: ModelArgs,
    /// The kinds to list: one or more, given again or separated by commas.
    #[arg(
        long,
        required_unless_present = "stray",
        value_delimiter = ',',
        value_parser = PossibleValuesParser::new(LISTED.map(EntityKind::text))
    )]
    pub kind: Vec<String>,
    /// Kinds written without `--kind`, which are refused with the flag to use, as the blind run's agents wrote them.
    #[arg(hide = true, value_name = "KIND")]
    pub stray: Vec<String>,
}

/// What the commands that take ids take.
#[derive(Debug, Args)]
pub struct IdsArgs {
    /// The model.
    #[command(flatten)]
    pub model: ModelArgs,
    /// The ids, each as kind:slot/generation, such as edge:41/1.
    #[arg(required = true, value_name = "ID", value_parser = entity)]
    pub ids: Vec<EntityRef>,
}

/// What `mesh` takes.
#[derive(Debug, Args)]
pub struct MeshArgs {
    /// The model.
    #[command(flatten)]
    pub model: ModelArgs,
    /// The farthest a triangle may stray from its face, in millimetres.
    #[arg(long, value_name = "LENGTH", value_parser = number)]
    pub deflection: f64,
    /// The most an edge may turn between two of its samples, in radians.
    #[arg(long, value_name = "ANGLE", value_parser = number, default_value_t = 0.35)]
    pub angle: f64,
    /// Where to write the buffers. Beside the model, as <model>.mesh, if not given.
    #[arg(long, value_name = "FILE")]
    pub out: Option<PathBuf>,
}

/// What `view` takes.
#[derive(Debug, Args)]
#[expect(clippy::struct_excessive_bools, reason = "clap reads each flag of the command line as a bool")]
pub struct ViewArgs {
    /// The model to show. It need not be there yet: the page draws it once a run writes it.
    #[arg(long, value_name = "FILE")]
    pub model: PathBuf,
    /// Say whether the model's viewer runs, and where, starting nothing.
    #[arg(long, conflicts_with = "stop")]
    pub status: bool,
    /// Stop the model's viewer.
    #[arg(long)]
    pub stop: bool,
    /// Do not open the page in a browser.
    #[arg(long)]
    pub no_open: bool,
    /// Listen on this port of 127.0.0.1, rather than on a free one.
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,
    /// Run as the viewer itself, which `occam view` starts.
    #[arg(long, hide = true)]
    pub serve: bool,
    /// How long the viewer runs with no page open before it stops itself.
    #[arg(long, hide = true, value_name = "SECONDS", default_value_t = 3600)]
    pub idle: u64,
}

/// What `distance` takes.
#[derive(Debug, Args)]
pub struct DistanceArgs {
    /// From this point, in millimetres.
    #[arg(long, value_name = "X,Y,Z", value_parser = numbers::<3>)]
    pub from: [f64; 3],
    /// To this point, in millimetres.
    #[arg(long, value_name = "X,Y,Z", value_parser = numbers::<3>)]
    pub to: [f64; 3],
    /// The tolerances the model is built to, a length in millimetres and an angle in radians.
    #[arg(long = "model-tolerance", value_name = "LENGTH,ANGLE", value_parser = numbers::<2>)]
    pub model_tolerance: [f64; 2],
    /// The tolerance asked for, in millimetres.
    #[arg(long, value_name = "LENGTH", value_parser = number)]
    pub tolerance: f64,
}

/// The id `text` writes.
fn entity(text: &str) -> Result<EntityRef, String> {
    EntityRef::parse(text).ok_or_else(|| ID_FORM.to_string())
}

/// The number `text` writes.
fn number(text: &str) -> Result<f64, String> {
    text.parse().map_err(|_| "a number".to_string())
}

/// The `N` numbers `text` lists, separated by commas.
fn numbers<const N: usize>(text: &str) -> Result<[f64; N], String> {
    let parsed: Result<Vec<f64>, _> = text.split(',').map(str::parse).collect();
    parsed.ok().and_then(|list| list.try_into().ok()).ok_or_else(|| format!("{N} numbers separated by commas"))
}

/// The kind `text` names, one of [`LISTED`].
pub fn kind(text: &str) -> Option<EntityKind> {
    LISTED.into_iter().find(|kind| kind.text() == text)
}
