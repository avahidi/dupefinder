mod hash;
use std::env;
use std::fs::{self, File};
use std::io::{self, SeekFrom};
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

const READSIZE : usize = 5;

#[derive(Hash,PartialEq,Eq, Debug)]
struct Item {
    path : PathBuf,
    size : u64,
    duplicate : bool,
    bytes : Option<[u8; 3 * READSIZE]>,
    hash: Option<hash::State>,
}

impl Item {
    fn new(path: PathBuf, size : u64) -> Self {
        Item { path, size, bytes: None, hash: None, duplicate: false, }
    }
    fn bytes(&mut self) -> io::Result<[u8;3 * READSIZE]> {
        if let Some(data) = self.bytes {
            return Ok(data);
        }

        let mut file = File::open(&self.path)?;
        let mut data = [0; 3 * READSIZE];
        if self.size < (3 * READSIZE) as u64 {
            let _ = file.read(& mut data)?;
        } else {
            file.read_exact(& mut data[0..READSIZE])?;
            file.seek(SeekFrom::Start( (self.size - READSIZE as u64) / 2))?;
            file.read_exact(& mut data[READSIZE..2*READSIZE])?;
            file.seek(SeekFrom::End(-(READSIZE as i64)))?;
            file.read_exact(& mut data[2*READSIZE..3*READSIZE])?;
        }

        self.bytes = Some(data);
        Ok(data)
    }

    fn hash(&mut self) -> io::Result<hash::State> {
        if let Some(data) = self.hash {
            return Ok(data);
        }

        let hash = hash::from_file(&self.path)?;
        self.hash = Some(hash);
        Ok(hash)
    }
}

pub struct Items {
    map: HashMap< u64, Vec<Item>>,
}

impl Items {
    fn new() -> Self { Items{ map: HashMap::new() } }

    fn len(&self) -> usize { self.map.len() }

    fn add(&mut self, size: u64, path: PathBuf) {
        let item = Item::new(path, size);
        self.map.entry(size).or_insert(Vec::new()).push(item);
    }

    fn scan(&mut self, path: &Path, follow_symlinks: bool) {
        if path.is_symlink() &&  !(path.is_dir() && follow_symlinks) {
            return // ignore this symlink
        }

        if path.is_dir() {
            if let Ok(listing) = fs::read_dir(path) {
                for entry in listing {
                    let path = entry.expect("Unknown error").path();
                    self.scan(&path, follow_symlinks);
                }
            }
        } else if let Ok(metadata) = fs::metadata(path) {
            let path = path.canonicalize().unwrap();
            self.add(metadata.len(), path );

            print!("{} files scanned    \r", self.len());
            let _ = std::io::stdout().flush();
        }
    }
}

enum Mode {
    Show,
    Delete,
    Command,
    Json,
}

struct Configuration {
    folders : Vec<String>,
    mode : Mode,
    follow_symlinks: bool,
}

fn process_duplicates(mode: Mode, dups: &HashMap<PathBuf, Vec<PathBuf>>) {
    if let Mode::Json = mode { println!("{{") }

    for (j, (org, copies)) in dups.iter().enumerate() {
        match mode {
            Mode::Show => println!("File {:?} has the following duplicates:", org),
            Mode::Command => print!("# deleting duplicates of {:?}...\nrm ", org),
            Mode::Delete => println!("Deleting copies of {:?}...", org),
            Mode::Json => print!("{}  {:?}: [", if j==0 {""} else {",\n"}, org),
        }
        for (i, copy) in copies.iter().enumerate() {
            match mode {
                Mode::Show => println!("\t{} - {:?}", i + 1, copy),
                Mode::Command => print!("{:?} ", copy),
                Mode::Delete => match fs::remove_file(copy) {
                    Ok(_) => println!("\tDeleted {:?}", copy),
                    Err(err) => eprintln!("\tCould not delete {:?}: {}", copy, err),
                }
                Mode::Json => print!("{}{:?}", if i == 0 {""} else {", "}, copy),
            }
        }

        match mode {
            Mode::Json => print!("]"),
            _ => println!(),
        }
    }

    if let Mode::Json = mode { println!("\n}}") }
}


fn help()  {
    let prog = env::args().next().unwrap();
    println!("Usage: {prog} [--followsymlinks] [--mode=mode] paths");
    println!("Valid modes are: show (default), delete, command, json");
}

fn parse_args() -> Result<Configuration, String> {
    let mut c = Configuration {
        folders: env::args().skip(1).filter(|x| !x.starts_with('-')).collect::<Vec<_>>(),
        mode: Mode::Show,
        follow_symlinks: false,
    };

    let args = env::args().skip(1).filter(|x| x.starts_with('-')).collect::<Vec<String>>();
    for arg in &args {
        let (option, data) = match arg.split_once('=') {
            Some(s) => (s.0, Some(s.1)),
            None => (&arg[..], None),
        };
        match option {
            "-h" | "--help" => {
                help();
                std::process::exit(0);
            }
            "-f" | "--followsymlinks" => c.follow_symlinks = true,
            "-m" | "--mode" => {
                match data {
                    Some("show") => c.mode = Mode::Show,
                    Some("delete") => c.mode = Mode::Delete,
                    Some("command") => c.mode = Mode::Command,
                    Some("json") => c.mode = Mode::Json,
                    _ => return Err( format!("Unknown or missing mode: {}", arg) ),
                };
            },
            _ => return Err(format!("Unknown parameter: {}", arg)),
        }
    }
    if c.folders.is_empty() {
        Err( String::from("No paths given"))
    } else {
        Ok( c)
    }
}

fn main() {
    let config = match parse_args() {
        Ok(c) => c,
        Err(s) => {
            help();
            println!("{}", s);
            std::process::exit(20);
        },
    };

    // scan files
    let mut items = Items::new();
    for entry in &config.folders {
        let path = Path::new(entry);
        items.scan(path, config.follow_symlinks);
    }
    eprintln!("Step 1: scanned {} files", items.len());

    // phase 1: remove sizes with only one file
    items.map = items.map.into_iter().filter( |(_,v)| v.len() > 1).collect();
    eprintln!("Step 2: {} possible duplicates based on size", items.len());

    // phase 2: find files that are duplicate according to byte extract
    let mut dups  = HashMap::new();

    for (_, mut list) in items.map {
        let len = list.len();
        for i in 0 .. len {
            if list[i].duplicate {
                continue
            }

            let bytesi = &list[i].bytes().expect("");
            for j in i+1 .. len {
                if list[i].path == list[j].path || list[j].duplicate {
                    continue
                }

                let bytesj = &list[j].bytes().expect("");
                if bytesi == bytesj {
                    // phase 3: find are files also identical according to digest?
                    let hashi = &list[i].hash().expect("");
                    let hashj = &list[j].hash().expect("");
                    if hashi == hashj {
                        list[j].duplicate = true;
                        dups.entry(list[i].path.clone() )
                            .or_insert_with(Vec::new)
                            .push(list[j].path.clone() );
                    }
                }
            }
        }
    }

    // now that we found duplicates, take care of them
    let duplicates : usize = dups.iter().map(|x| x.1.len()).sum();
    eprintln!("Step 3: based on content {} files have a total of {} duplicates",dups.len(), duplicates);

    process_duplicates(config.mode, &dups);
}
