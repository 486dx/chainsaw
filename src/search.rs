use crate::util::{check_output_file, get_evtx_files, get_progress_bar, json_write_to_file};
use anyhow::Result;
use evtx::{EvtxParser, ParserSettings};
use regex::Regex;
use std::fs::File;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct SearchOpts {
	/// Specify an EVTX file, or a directory containing the EVTX files to search.
	/// If you specify a directory, all files matching *.evtx will be used.
	pub evtx_path: PathBuf,

	/// Save the output of the search filters to a specified path
	#[structopt(short = "o", long = "output")]
	pub output_file: Option<PathBuf>,

	/// This option can be used in conjunction with any other search methods. It will only return
	/// results of the event ID supplied.
	#[structopt(short = "e", long = "event")]
	pub event_id: Option<u32>,

	/// Use this option to search EVTx files for the string supplied. If the string is found, the
	/// whole matching event will be returned.
	/// Use the -i flag for case insensitive searches.
	#[structopt(short = "s", long = "string")]
	pub search_string: Option<String>,

	/// Set search to case insensitive. Usable only with string searching.
	#[structopt(short = "i", long = "case-insensitive")]
	pub case_insensitive: bool,

	/// Use this option to search EVTx files for the regex pattern supplied. If a pattern match is found, the
	/// whole matching event will be returned.
	#[structopt(short = "r", long = "regex-search")]
	pub search_regex: Option<String>,
}

pub fn run_search(opt: SearchOpts) -> Result<String> {
	// Load EVTX Files
	let evtx_files = get_evtx_files(&opt.evtx_path)?;
	// Perform sanity checks on output file
	match &opt.output_file {
		Some(file) => {
			check_output_file(file)?;
			println!("[+] Saving results to {:?}", file);
		}
		None => println!("[+] Printing results to the screen"),
	}
	let pb = get_progress_bar(evtx_files.len() as u64, "Searching".to_string());
	// Loop through EVTX files and perform actions
	let mut count = 0;
	for evtx in &evtx_files {
		pb.tick();
		// Parse EVTx files
		let settings = ParserSettings::default().num_threads(0);
		let parser = EvtxParser::from_path(evtx)?.with_configuration(settings);

		// Search EVTX files for user supplied arguments
		count += search_evtx_file(parser, &opt)?;
		pb.inc(1);
	}
	pb.finish();
	Ok(format!("\n[+] Found {} matching log entries", count))
}

pub fn search_evtx_file(mut parser: EvtxParser<File>, opt: &SearchOpts) -> Result<i64> {
	let mut count = 0;
	for record in parser.records_json_value() {
		// TODO - work out why chunks of a record can fail here, but the overall event logs count
		// isn't affected. If this parser isn't seeing an event that you know exists, it's mostly
		// likely due to this match block
		let r = match record {
			Ok(record) => record,
			Err(_) => {
				continue;
			}
		};
		// Do processing of EVTX record now it's in a JSON format
		//
		// The default action of the whole OK logic block it mark a record as matched
		// If a filter criteria is NOT matched, then we contiue the loop and don't push the
		// Record onto the matched records array

		// EventIDs can be stored in two different locations
		let event_id;
		if r.data["Event"]["System"]["EventID"]["#text"].is_null() {
			event_id = &r.data["Event"]["System"]["EventID"];
		} else {
			event_id = &r.data["Event"]["System"]["EventID"]["#text"];
		}

		// Handle event_id search option
		if let Some(e_id) = opt.event_id {
			if event_id != e_id {
				continue;
			}
		};
		// Handle string search option
		if let Some(string) = &opt.search_string {
			if opt.case_insensitive {
				// Case insensitive string search
				if !r
					.data
					.to_string()
					.to_lowercase()
					.contains(&string.to_lowercase())
				{
					continue;
				}
			} else {
				// Case sensitive search
				if !r.data.to_string().contains(string) {
					continue;
				}
			}
		};

		// Handle regex search option
		if let Some(reg) = &opt.search_regex {
			let re = Regex::new(reg)?;
			if !re.is_match(&r.data.to_string()) {
				continue;
			}
		}

		count += 1;

		if let Some(out_file) = &opt.output_file {
			if let Err(e) = json_write_to_file(out_file, &r.data) {
				return Err(anyhow!("Could not write output to file: {}", e));
			}
		} else {
			let g = serde_json::to_string_pretty(&r.data)?;
			println!("{}", g);
		}
	}
	Ok(count)
}
