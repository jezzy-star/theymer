use std::fs;
use std::path::Path;

use clap::{ArgAction, Parser};
use env_logger::Builder as LoggerBuilder;
use log::{LevelFilter as LogLevelFilter, info};

use crate::output::WriteMode;
use crate::templates::Loader;
use crate::themes::schemes;
use crate::{Result, config, render};

// TODO: better documentation
// TODO: add `prune` flag
#[expect(clippy::struct_excessive_bools, reason = "cli args")]
#[derive(Debug, Clone, Parser)]
#[command(name = "they", version, about, long_about = None)]
struct Args {
    /// Output more info per invocation (-v, -vv, -vvv)
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,

    /// Silence all output except errors
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,

    /// Don't overwrite existing files
    #[arg(short, long)]
    keep: bool,

    /// Delete current contents of output directory before rendering
    #[arg(short, long, conflicts_with = "keep")]
    clean: bool,

    /// Overwrite all existing files, even user-modified ones
    #[arg(short, long, conflicts_with = "keep")]
    force: bool,

    /// Preview changes without writing them to disk
    #[arg(long, alias = "dry")]
    dry_run: bool,
}

impl Args {
    const fn write_mode(&self) -> WriteMode {
        // TODO: show files that would be generated/pruned in `dry_run` mode
        if self.force || self.clean {
            WriteMode::Force
        } else if self.keep {
            WriteMode::Skip
        } else {
            WriteMode::Smart
        }
    }
}

fn init_logger(verbosity: u8, quiet: bool) {
    let level = if quiet {
        LogLevelFilter::Error
    } else {
        match verbosity {
            0 => LogLevelFilter::Warn,
            1 => LogLevelFilter::Info,
            2 => LogLevelFilter::Debug,
            _ => LogLevelFilter::Trace,
        }
    };

    LoggerBuilder::new().filter_level(level).init();
}

pub fn run() -> Result<()> {
    let cli = Args::parse();

    init_logger(cli.verbose, cli.quiet);

    let config = config::load()?;

    let templates = Loader::init(&config)?;
    let schemes = schemes::load_all(&config.dirs.schemes)?;

    if cli.clean {
        let render_dir = Path::new(&config.dirs.render);
        if render_dir.exists() {
            if cli.dry_run {
                info!("would clean `{}`", render_dir.display());
            } else {
                fs::remove_dir_all(render_dir)?;
                info!("cleaned `{}`", render_dir.display());
            }
        }
    }

    render::all(&templates, &schemes, &config, cli.write_mode(), cli.dry_run)?;

    Ok(())
}
