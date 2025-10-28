#![allow(dead_code)]

mod ai;
mod app;
mod beatport;
mod convert;
mod fields;
mod file_utils;
mod keyfinder;
mod logging;
mod media_hash;
mod processing;
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
