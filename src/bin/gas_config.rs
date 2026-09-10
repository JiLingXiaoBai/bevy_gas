//! Command-line validation, inspection, and manifest creation for configuration packages.

use bevy::prelude::*;
use bevy_gas::GameplayAbilitySystemPlugin;
use bevy_gas::config::{
    AbilityId, compile_catalog, describe_ability_at_level, load_tables, write_package_manifest,
};
use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::process::ExitCode;

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [command, directory] if command == "write-manifest" => {
            write_package_manifest(directory)?;
            io::stdout().lock().write_all(b"Package manifest written.\n")?;
        }
        [command, directory] if command == "validate-config" => {
            let tables = load_tables(directory)?;
            let mut app = App::new();
            app.add_plugins(MinimalPlugins).add_plugins(GameplayAbilitySystemPlugin);
            let catalog = compile_catalog(&tables, app.world_mut())?;
            writeln!(io::stdout().lock(), "Configuration validation passed: {} abilities, {} effects.", catalog.abilities().count(), catalog.effect_count())?;
        }
        [command, directory, ability] if command == "inspect" => inspect(directory, ability, 1)?,
        [command, directory, ability, level] if command == "inspect" => inspect(directory, ability, level.parse()?)?,
        _ => return Err("usage: gas-config write-manifest <data-directory> | validate-config <data-directory> | inspect <data-directory> <ability-id> [level]".into()),
    }
    Ok(())
}

fn inspect(directory: &str, ability: &str, level: u32) -> Result<(), Box<dyn Error>> {
    let id = AbilityId(ability.parse()?);
    let tables = load_tables(directory)?;
    io::stdout()
        .lock()
        .write_all(describe_ability_at_level(&tables, id, level)?.as_bytes())?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "{error}");
            ExitCode::FAILURE
        }
    }
}
