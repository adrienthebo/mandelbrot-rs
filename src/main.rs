extern crate nalgebra;
extern crate num;
extern crate serde;
extern crate structopt;
extern crate termion;
extern crate tui;

use indicatif::ProgressBar;
use mandelbrot::frontend;
use mandelbrot::rctx::Rctx;
use mandelbrot::{loc::Loc, Bounds, Error};
use std::fs::File;
use std::io::Read;
use std::time::Instant;
use structopt::StructOpt;

#[derive(Debug)]
enum FrontendType {
    Termion,
    Tui,
}

#[derive(Debug)]
pub struct FrontendTypeParseError(String);

impl std::fmt::Display for FrontendTypeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Cannot parse {} to frontend", self.0)
    }
}

impl std::error::Error for FrontendTypeParseError {}

impl std::str::FromStr for FrontendType {
    type Err = FrontendTypeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tui" => Ok(FrontendType::Tui),
            "termion" => Ok(FrontendType::Termion),
            _ => Err(FrontendTypeParseError(s.to_string())),
        }
    }
}

fn read_rctx(path: &std::path::PathBuf) -> std::result::Result<Rctx, crate::Error> {
    let mut buf = String::new();
    File::open(&path)
        .map_err(|e| e.into())
        .and_then(|mut fh| {
            fh.read_to_string(&mut buf)
                .map_err(|e| crate::Error::from(e))
        })
        .and_then(|_| serde_json::from_str(&buf).map_err(|e| e.into()))
}

#[derive(Debug, StructOpt)]
#[structopt(name = "mandelbrot")]
struct AppOptions {
    #[structopt(long = "spec")]
    spec: Option<std::path::PathBuf>,
}

#[derive(Debug, StructOpt)]
enum Subcommand {
    #[structopt(name = "run")]
    Run {
        #[structopt(long = "frontend")]
        frontend_type: Option<FrontendType>,

        #[structopt(long = "spec")]
        spec: Option<std::path::PathBuf>,

        #[structopt(long = "img-dir")]
        img_dir: Option<std::path::PathBuf>,
    },

    #[structopt(name = "render")]
    Render {
        spec: std::path::PathBuf,

        #[structopt(long = "dest")]
        dest: Option<std::path::PathBuf>,

        #[structopt(long = "height", default_value = "4000")]
        height: u16,

        #[structopt(long = "width", default_value = "4000")]
        width: u16,
    },
}

#[derive(Debug, StructOpt)]
struct Command {
    #[structopt(subcommand)]
    subcommand: Subcommand,
}

/// Run an interactive mandelbrot explorer
fn run(
    frontend_type: Option<FrontendType>,
    spec: Option<std::path::PathBuf>,
    img_dir: Option<std::path::PathBuf>,
) -> std::result::Result<(), crate::Error> {
    let mut rctx: Rctx;
    if let Some(ref path) = spec {
        rctx = read_rctx(&path)?;
    } else {
        rctx = Rctx::with_loc(Loc::for_bounds(termion::terminal_size()?.into()));
    }
    rctx.comp = (2.3, 1.0);

    let mut runtime: Box<dyn mandelbrot::frontend::Frontend> = match frontend_type {
        None | Some(FrontendType::Termion) => Box::new(mandelbrot::frontend::Termion {}),
        Some(FrontendType::Tui) => Box::new(mandelbrot::frontend::Tui {})
    };

    frontend::run_with_altscreen(move || runtime.run(rctx, frontend::RunOptions::new(img_dir)))
}

#[allow(unused)]
fn time_fn<T, U>(desc: &str, f: T) -> U
where
    T: FnOnce() -> U,
{
    let start = Instant::now();
    let result: U = f();
    println!("{} elapsed: {:?}", desc, start.elapsed());
    result
}

/// Render a fractal from the given spec/rctx
fn render(
    spec: std::path::PathBuf,
    height: u16,
    width: u16,
    dest: Option<std::path::PathBuf>,
) -> std::result::Result<(), crate::Error> {
    let mut rctx = read_rctx(&spec)?;
    rctx.comp = (1., 1.);
    let bound_rctx = rctx.bind(Bounds {
        height: height,
        width: width,
    });

    // XXX bad conversion
    let bar = ProgressBar::new(0);

    bar.set_style(
        indicatif::ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {percent}% {wide_bar:cyan/blue} {pos:>7}/{len:7} ({per_sec}) {msg} [eta: {eta_precise}]")
    );
    bar.set_draw_delta(10000);

    let output_path = dest.unwrap_or(spec.with_extension("png"));

    //let ematrix = time_fn("ematrix", || bound_rctx.to_ematrix_with_bar(bar));
    let ematrix = time_fn("ematrix", || bound_rctx.to_ematrix());
    let img = time_fn("coloring", || ematrix.to_img(&rctx.colorer));
    img.save(&output_path).map_err(|e| Error::from(e))
}

fn main() -> std::result::Result<(), crate::Error> {
    let cmd = Command::from_args();

    match cmd.subcommand {
        Subcommand::Run {
            frontend_type,
            spec,
            img_dir,
        } => run(frontend_type, spec, img_dir),
        Subcommand::Render {
            spec,
            height,
            width,
            dest,
        } => render(spec, height, width, dest),
    }
}
