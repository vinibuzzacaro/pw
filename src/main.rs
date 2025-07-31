use std::{collections::HashSet, fs, io::{self, Read}};

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use copypasta::{ClipboardContext, ClipboardProvider};
use keyring::Entry;
use serde::{Deserialize, Serialize};

const FILE_DIR: &'static str = "./keys.json";
const SERVICE: &'static str = "pw-cli";

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    key: Option<String>,
    #[arg(short, long, global(true))]
    copy: bool,
    #[arg(short, long, global(true))]
    quiet: bool,    
    #[arg(short, long, global(true))]
    verbose: bool
}

#[derive(Subcommand)]
enum Commands {
    Set {
        key: String,
        password: String
    },
    #[command(name("rm"))]
    Remove { key: String },
    List
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct KeyStorage {
    keys: HashSet<String>
}

impl KeyStorage {
    fn load() -> io::Result<Self> {
        if !fs::metadata(FILE_DIR).is_ok() {
            return Ok(Self::default())
        }
        let file = fs::File::open(FILE_DIR)?;
        serde_json::from_reader(file)
            .map_err(|e| io::Error::from(e))
    }

    fn save(&self) -> io::Result<()> {
        let file = fs::File::create(FILE_DIR)?;
        serde_json::to_writer_pretty(file, self)
            .map_err(|e| io::Error::from(e))
    }
}

impl Cli {
    fn entry(key: &str) -> keyring::Result<Entry> {
        Entry::new_with_target(key, SERVICE, &whoami::username())
    }

    fn handle_set_command(key: String, password: String, quiet: bool) -> anyhow::Result<()> {
        let mut storage = KeyStorage::load()?;                
        let entry = Self::entry(&key)?;
        entry.set_password(&password)?;
        if !quiet {
            println!("Password for \"{key}\" set successfully.");
        }
        if storage.keys.insert(key) {
            storage.save()?;            
        }                        
        Ok(())
    }

    fn handle_password(key: String, quiet: bool, copy: bool) -> anyhow::Result<()> {
        let entry = Self::entry(&key)?;
        let pw = entry.get_password()?;                
        if !quiet {
            println!("The password for \"{key}\" is \"{pw}\".");
        }
        if copy {
            let mut ctx = ClipboardContext::new()
                .map_err(|e| anyhow!("Failed to initialize clipboard: {e}"))?;
            ctx.set_contents(pw)
                .map_err(|e| anyhow!("Failed to set clipboard content: {e}"))?;
            if !quiet {
                println!("Copied to the clipboard!");
            }

        }         
        Ok(())
    }

    fn handle_list_command(quiet: bool) -> anyhow::Result<()> {
        if !quiet {            
            let storage = KeyStorage::load()
                .unwrap_or_default();
            if !storage.keys.is_empty() {
                let keys = storage.keys
                    .into_iter()
                    .map(|s| format!("* {s}"))
                    .collect::<Vec<String>>()
                    .join("\n");
                println!("{keys}");
            }        
        }        
        Ok(())
    }

    fn handle_remove_command(key: String, quiet: bool) -> anyhow::Result<()> {
        let mut storage = KeyStorage::load()?;
        if storage.keys.remove(&key) {
            let entry = Self::entry(&key)?;                     
            entry.delete_credential()?;                
            storage.save()?;
        }
        if !quiet {
            println!("\"{key}\" removed successfully.");
        }  
        Ok(())
    }
}

fn main() {
    let cli = Cli::parse();
    let err = match cli.command {
        Some(Commands::Set { key, password }) => Cli::handle_set_command(key, password, cli.quiet),
        Some(Commands::List) => Cli::handle_list_command(cli.quiet),
        Some(Commands::Remove { key}) => Cli::handle_remove_command(key, cli.quiet),
        None => match cli.key {
            Some(key) => Cli::handle_password(key, cli.quiet, cli.copy),
            None => return,
        }
    };
    if !cli.quiet { 
        if let Err(e) = err {
           eprintln!("An error has occured! Error: {e}");
        }
    }    
}
