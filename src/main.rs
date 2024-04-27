
use std::fs;
use std::env;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

enum Mode {
    Show,
    Delete,
    Command,
}

struct Configuration {
    folders : Vec<String>,
    mode : Mode,
}

struct Item {
    path : PathBuf,
}
impl Item {
    fn new(path: PathBuf) -> Self {
	Item { path }
    }
}

struct Items {
    map: HashMap< u64, Vec<Item>>,
}

impl Items {
    fn new() -> Self { Items{ map: HashMap::new() } }
    fn add(&mut self, size: u64, path: PathBuf) {
	let item = Item::new(path);
	self.map.entry(size).or_insert(Vec::new()).push(item);
    }
}



fn scan_path(path: &Path, items: &mut Items ) {
    if path.is_dir() {
	if let Ok(listing) = fs::read_dir(path) {
	    for entry in listing {
		let path = entry.expect("Unknown error").path();
		scan_path(&path, items);
	    }
	}
    } else {
	if let Ok(metadata) =  fs::metadata(path) {
	    items.add(metadata.len(), path.to_owned() );

	}
    }
}
fn args_help() -> ! {
    let prog = env::args().next().unwrap();
    println!("Usage: {prog} [-m=mode] paths");
    println!("Valid modes are: show (default), delete, command");
    std::process::exit(0)
}

fn args_parse() -> Result<Configuration, String> {
    let mut c = Configuration {
        folders: env::args().skip(1).filter(|x| !x.starts_with("-")).collect::<Vec<_>>(),
        mode: Mode::Show,
    };

    let options = env::args().skip(1).filter(|x| x.starts_with("-")).collect::<Vec<String>>();
    for option in &options {
        let (option, data) : (&str, Option<&str>)= match option.split_once("=") {
            Some(s) => (s.0, Some(s.1)),
            None => (&option[..], None),
        };
        match &option [..] {
            "-h" | "--help" => args_help(),
            "-m" | "--mode" => {
                match data {
                    Some("show") => c.mode = Mode::Show,
                    Some("delete") => c.mode = Mode::Delete,
                    Some("command") => c.mode = Mode::Command,
                    _ => return Err( String::from("Unknown or missing mode") ),
                };
            },
            s => return Err(format!("Unknown option: {}", s)),
        }
    }

    if c.folders.len() == 0 {
        Err( String::from("No paths given"))
    } else {
        Ok( c)
    }
}



fn main() {
    let config = match args_parse() {
        Ok(c) => c,
        Err(s) => {
            println!("{}", s);
            std::process::exit(20);
        },
    };

    println!("config={:?}", config.folders);

    let mut items = Items::new();
    for entry in &config.folders {
	let path = Path::new(entry);
	scan_path(&path, &mut items);
    }


}
