use failure::Error;
use std::{collections::HashMap, env, fs::File, io::{self, Read, BufRead, BufReader}};
use serde::{Deserialize, Serialize};
// import coverage
use mem_analysis::module_analysis::coverage::Cover;


#[derive(Debug, Serialize, Deserialize)]
struct JsonData {
    covered_num: u64,
    total_coverage: u64,
    exec_num: u64,
    timestamp: u64,
    cov_map: HashMap<String, Range>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Range {
    start: u64,
    end: u64,
}
 

fn main() -> Result<(), failure::Error> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} <binary_file> <json_file>", args[0]);
        std::process::exit(1);
    }

    let binary_file = &args[1];
    let json_file = &args[2];

    let mut binary_data = Vec::new();
    File::open(binary_file)?.read_to_end(&mut binary_data)?;

    let file = File::open(json_file)?;
    let reader = BufReader::new(file);

    let mut last_cov_map: Option<HashMap<String, Range>> = None;

    for line in reader.lines() {
        let line = line?;
        let deserialized_data: Result<JsonData, serde_json::Error> = serde_json::from_str(&line);
        if let Ok(data) = deserialized_data {
            last_cov_map = Some(data.cov_map);
        }
    }

    if let Some(cov_map) = last_cov_map {
        // Access last cov_map here
        println!("Last cov_map: {:?}", cov_map);

        for (key, range) in cov_map.iter() {
            let start = range.start as usize;
            let end = range.end as usize;

            if end <= binary_data.len() && start <= end {
                let slice = &binary_data[start..end];
                let mod_cover = slice.iter().filter(|&&x| x == 1);
                // wrire mod_cover to differnet file
                let file_name = format!("/etc/shm/mod_cover/mod_{}", key);
                let mut file = File::create(file_name)?;
                for i in mod_cover {
                    file.write_all(i.to_string().as_bytes())?;
                }

                // Do something with slice
                // let count_ones = slice.iter().filter(|&&x| x == 1).count();
                // println!("{} from {} to {}: {}", key, start, end, count_ones);
            } else {
                println!("{}: Start and end indices are out of range.", key);
            }
        }
    } else {
        println!("No cov_map data was available.");
    }

    Ok(())
}
