use std::{collections::HashSet, fs, io::{self}};

use anyhow::anyhow;
use arboard::Clipboard;
use clap::{Parser, Subcommand};
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
    tag: Option<String>
}

#[derive(Subcommand)]
enum Commands {
    Set {
        key: String,
        password: String,
        #[arg(short, long, global(true))]
        tag: Option<String>
    },
    #[command(name("rm"))]
    Remove { 
        key: String, 
        #[arg(short, long, global(true))]
        tag: Option<String> 
    },
    List { 
        #[arg(short, long, global(true))]
        tag: Option<String>,
        #[arg(long("no-tag"), conflicts_with("tag"))]
        no_tag: bool
    } 
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct KeyStorage {
    keys: HashSet<(String, Option<String>)>
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

    fn handle_set_command(key: String, password: String, quiet: bool, tag: Option<String>) -> anyhow::Result<()> {
        let mut storage = KeyStorage::load()?;                
        let entry_name = match &tag {
            Some(tag) => format!("{key}:{tag}"),
            None => key.to_string(),
        };
        let entry = Self::entry(&entry_name)?;
        entry.set_password(&password)?;                        
        if storage.keys.insert((key, tag)) {
            storage.save()?;
        }
        if !quiet {
            println!("Password for \"{entry_name}\" set successfully.");
        }
        Ok(())
    }

    fn handle_password(key: String, quiet: bool, copy: bool, tag: Option<String>) -> anyhow::Result<()> {        
        let matched_keys = KeyStorage::load()?.keys
            .iter()
            .filter_map(|(k, t)| (k == &key).then(|| {                
                match &tag {
                    Some(tag) => ((k.to_string(), t.to_owned()), format!("{key}:{tag}")),
                    None => match t {
                        Some(tag_str) => ((k.to_string(), t.to_owned()), format!("{key}:{tag_str}")),
                        None => ((k.to_string(), t.to_owned()), k.to_string()),
                    }
                }
            }))
            .collect::<Vec<((String, Option<String>), String)>>();        
        if matched_keys.len() > 1 && tag.is_none() {
            let matches = matched_keys
                .into_iter()
                .map(|(_, key)| format!("* {key}"))
                .collect::<Vec<String>>()
                .join("\n");            
            println!("Multiple matches found for \"{key}\":\n{matches}\nUse --tag/-t to specify which one to read from");
            return Ok(())
        }
        let Some((_, entry_name)) = matched_keys.first() else {
            panic!("Key \"{key}\" was found initially, but returned None.");
        };
        let entry = Self::entry(&entry_name)?;
        let pw = entry.get_password()?;                
        if !quiet {
            println!("The password for \"{entry_name}\" is \"{pw}\".");
        }
        if copy {
            let mut clipboard = Clipboard::new()
                .map_err(|e| anyhow!("Failed to initialize clipboard: {e}"))?;
            clipboard.set_text(pw)
                .map_err(|e| anyhow!("Failed to set clipboard content: {e}"))?;
            if !quiet {
                println!("Copied to the clipboard!");
            }
        }
        Ok(())
    }

    fn handle_list_command(quiet: bool, tag: Option<String>, no_tag: bool) -> anyhow::Result<()> {
        if !quiet {            
            let storage = KeyStorage::load()
                .unwrap_or_default();
            if !storage.keys.is_empty() {
                let filtered_keys = match (&tag, &no_tag) {
                    (Some(_), false) => storage.keys
                        .into_iter()
                        .filter_map(|(k, t)| (&t == &tag).then(|| (k, t)))
                        .collect::<Vec<(String, Option<String>)>>(),
                    (_, true) => storage.keys
                        .into_iter()
                        .filter_map(|(k, t)| t.is_none().then(|| (k, t)))
                        .collect::<Vec<(String, Option<String>)>>(),
                    (None, false) => storage.keys.into_iter().collect(),                    
                };
                let keys = filtered_keys
                    .into_iter()                    
                    .map(|(key, tag)| {
                        if let Some(tag) = tag {
                            format!("* {key}:{tag}")
                        } else {
                            format!("* {key}")
                        }
                    })
                    .collect::<Vec<String>>()
                    .join("\n");
                println!("{keys}");
            }        
        }
        Ok(())
    }

    fn handle_remove_command(key: String, quiet: bool, tag: Option<String>) -> anyhow::Result<()> {        
        let mut storage = KeyStorage::load()?;        
        let matched_keys = storage.keys
            .iter()
            .filter_map(|(k, t)| (k == &key).then(|| {                
                match &tag {
                    Some(tag) => ((k.to_string(), t.to_owned()), format!("{key}:{tag}")),
                    None => match t {
                        Some(tag_str) => ((k.to_string(), t.to_owned()), format!("{key}:{tag_str}")),
                        None => ((k.to_string(), t.to_owned()), k.to_string()),
                    }
                }
            }))
            .collect::<Vec<((String, Option<String>), String)>>();        
        if matched_keys.len() > 1 && tag.is_none() {
            let matches = matched_keys
                .into_iter()
                .map(|(_, key)| format!("* {key}"))
                .collect::<Vec<String>>()
                .join("\n");            
            println!("Multiple matches found for \"{key}\":\n{matches}\nUse --tag/-t to specify which one to remove");
            return Ok(())
        }    
        let msg = match matched_keys.first() {
            Some((storage_key, key_str)) => {
                if !storage.keys.remove(&storage_key) {
                    panic!("Key found in storage previously failed on removal.");
                }
                let entry = Self::entry(&key_str)?;
                entry.delete_credential()?;
                storage.save()?;
                format!("\"{key_str}\" removed successfully.")
            },
            None => format!("\"{key}\" not found."),                     
        };
        if !quiet {
            println!("{msg}");
        }
        Ok(())
    }
}

fn main() {
    let cli = Cli::parse();
    let err = match cli.command {
        Some(Commands::Set { key, password, tag }) 
            => Cli::handle_set_command(key, password, cli.quiet, tag),
        Some(Commands::List { tag, no_tag }) 
            => Cli::handle_list_command(cli.quiet, tag, no_tag),
        Some(Commands::Remove { key, tag }) 
            => Cli::handle_remove_command(key, cli.quiet, tag),
        None => match cli.key {
            Some(key) => Cli::handle_password(key, cli.quiet, cli.copy, cli.tag),
            None => return,
        }
    };
    if !cli.quiet { 
        if let Err(e) = err {
           eprintln!("An error has occured! Error: {e}");
        }
    }    
}
