#![allow(dead_code)]

mod app;
mod beatport;
mod fields;
mod importer;
mod logging;
mod tags;
mod track;

#[cfg(test)]
mod tests;

use std::process;

use sentry::Hub;

fn main() {
    let exit_code = match app::execute() {
        Ok(_) => 0,
        Err(_) => 1,
    };

    Hub::current().client().map(|x| x.close(None));
    process::exit(exit_code);
}
