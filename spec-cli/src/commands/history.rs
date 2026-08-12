use std::path::Path;

use anyhow::Result;

use crate::history::generate;

pub fn run(specs_dir: &Path, update: bool, id: Option<&str>) -> Result<()> {
    if update {
        let written = generate::update_history(specs_dir)?;
        if written.is_empty() {
            println!("History is up to date.");
        } else {
            println!("Updated history for {} spec(s):", written.len());
            for id in &written {
                println!("  {id}");
            }
        }
        return Ok(());
    }

    if let Some(spec_id) = id {
        match generate::read_history(specs_dir, spec_id)? {
            Some(history) => {
                println!("History for {}:", history.id);
                for event in &history.events {
                    println!(
                        "  {} {} ({}) by {} on {}",
                        event.sha, event.kind, history.id, event.author, event.date
                    );
                }
            }
            None => {
                println!("No history found for '{spec_id}'");
            }
        }
        return Ok(());
    }

    // List all history files
    let history_dir = specs_dir.join("_history");
    if history_dir.exists() {
        let mut entries: Vec<_> = std::fs::read_dir(&history_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
            .collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if let Ok(history) = serde_json::from_str::<generate::HistoryFile>(&content) {
                    println!("  {} ({} events)", history.id, history.events.len());
                }
            }
        }
    } else {
        println!("No history directory found. Run `spec history rebuild` to generate.");
    }

    Ok(())
}
