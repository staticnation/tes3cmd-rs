//! Where tes3cmd's messages go: `print` to standard output, `warn` (its `msg`, `err`, `dbg`) to
//! standard error, each line as tes3cmd words it. A test hands in buffers instead.

use std::io::Write;

/// A failure that stops the command for this plugin (tes3cmd's `abort`: `FATAL ERROR (plugin):`).
#[derive(Debug, Clone, PartialEq)]
pub struct Abort(pub String);

pub type Res<T> = Result<T, Abort>;

pub struct Ctx<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    /// `$CURRENT_PLUGIN`, named in errors.
    pub plugin: String,
    pub verbose: bool,
    pub debug: bool,
}

impl<'a> Ctx<'a> {
    pub fn new(out: &'a mut dyn Write, err: &'a mut dyn Write) -> Self {
        Ctx {
            out,
            err,
            plugin: String::new(),
            verbose: false,
            debug: false,
        }
    }
    /// `prn` / `print`: standard output, as given (the caller ends the line).
    pub fn print(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
    }
    /// `msg`: a line to standard error.
    pub fn msg(&mut self, s: &str) {
        let _ = writeln!(self.err, "{s}");
    }
    /// `err`: `[ERROR (plugin): ...]` to standard error.
    pub fn error(&mut self, s: &str) {
        let _ = writeln!(self.err, "[ERROR ({}): {s}]", self.plugin);
    }
    pub fn dbg(&mut self, s: &str) {
        if self.debug {
            let _ = writeln!(self.err, "[DEBUG ({}): {s}]", self.plugin);
        }
    }
    /// `abort`'s message, for a caller that reports it.
    pub fn abort(&self, s: &str) -> Abort {
        Abort(format!("FATAL ERROR ({}): {s}", self.plugin))
    }
}
