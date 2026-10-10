use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;
use colored::*;
use rayon::prelude::*;
use regex::Regex;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about = "⚡ Быстрый многопоточный поисковик файлов на Rust", long_about = None)]
struct Args {
    /// Шаблон имени файла (подстрока или регулярное выражение)
    #[arg(short, long, default_value = "")]
    name: String,

    /// Начальная папка для поиска
    #[arg(short, long, default_value = ".")]
    path: String,

    /// Фильтр по расширению (например, rs, txt, json)
    #[arg(short, long)]
    ext: Option<String>,

    /// Искать подстроку/текст внутри файлов
    #[arg(short, long)]
    content: Option<String>,

    /// Минимальный размер файла в килобайтах (КБ)
    #[arg(long)]
    min_size_kb: Option<u64>,

    /// Максимальный размер файла в килобайтах (КБ)
    #[arg(long)]
    max_size_kb: Option<u64>,
}

struct MatchResult {
    path: PathBuf,
    size_bytes: u64,
    content_matches: Vec<(usize, String)>,
}

fn main() {
    let args = Args::parse();
    let start_time = Instant::now();

    println!("{}", "========================================".cyan());
    println!("{}", "       ⚡ RUSTY-FIND SEARCH TOOL ⚡       ".cyan().bold());
    println!("{}", "========================================".cyan());
    println!("🔍 Поиск в директории: {}", args.path.yellow().bold());
    if !args.name.is_empty() {
        println!("📝 По шаблону имени:   {}", args.name.green());
    }
    if let Some(ref ext) = args.ext {
        println!("📎 Расширение:         .{}", ext.green());
    }
    if let Some(ref content) = args.content {
        println!("🔎 Поиск внутри файла: \"{}\"", content.magenta());
    }
    println!("{}", "----------------------------------------".dimmed());

    let name_regex = if !args.name.is_empty() {
        Regex::new(&format!("(?i){}", args.name)).ok()
    } else {
        None
    };

    let content_regex = args.content.as_ref().and_then(|c| Regex::new(&format!("(?i){}", c)).ok());

    // Быстрый сбор списка файлов
    let root_path = Path::new(&args.path);
    if !root_path.exists() {
        eprintln!("{}", format!("❌ Ошибка: путь '{}' не существует!", args.path).red());
        return;
    }

    let entries: Vec<PathBuf> = WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    let total_scanned = entries.len();

    // Параллельная фильтрация через Rayon
    let matched_files: Vec<MatchResult> = entries
        .into_par_iter()
        .filter_map(|path| {
            // Фильтр по расширению
            if let Some(ref target_ext) = args.ext {
                let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !file_ext.eq_ignore_ascii_case(target_ext) {
                    return None;
                }
            }

            // Фильтр по имени
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some(ref reg) = name_regex {
                if !reg.is_match(filename) {
                    return None;
                }
            }

            // Фильтр по размеру
            let metadata = path.metadata().ok()?;
            let size_bytes = metadata.len();
            let size_kb = size_bytes / 1024;

            if let Some(min_kb) = args.min_size_kb {
                if size_kb < min_kb {
                    return None;
                }
            }
            if let Some(max_kb) = args.max_size_kb {
                if size_kb > max_kb {
                    return None;
                }
            }

            // Поиск текста внутри файла (если запрошено)
            let mut line_matches = Vec::new();
            if let Some(ref reg) = content_regex {
                if let Ok(file) = File::open(&path) {
                    let reader = BufReader::new(file);
                    for (line_idx, line_res) in reader.lines().enumerate() {
                        if let Ok(line) = line_res {
                            if reg.is_match(&line) {
                                line_matches.push((line_idx + 1, line.trim().to_string()));
                                if line_matches.len() >= 5 {
                                    break; // ограничиваем вывод первыми 5 совпадениями на файл
                                }
                            }
                        }
                    }
                }
                if line_matches.is_empty() {
                    return None; // если искали по контенту и ничего не нашли
                }
            }

            Some(MatchResult {
                path,
                size_bytes,
                content_matches: line_matches,
            })
        })
        .collect();

    let duration = start_time.elapsed();

    // Вывод результатов
    for m in &matched_files {
        let size_str = format_size(m.size_bytes);
        println!("📄 {} [{}]", m.path.display().to_string().cyan(), size_str.yellow());
        for (line_no, line_text) in &m.content_matches {
            println!("   ↳ строка {}: {}", line_no.to_string().dimmed(), line_text.white());
        }
    }

    println!("{}", "----------------------------------------".dimmed());
    println!(
        "✨ Найдено: {} файлов (просканировано: {}) за {:.2} мс",
        matched_files.len().to_string().green().bold(),
        total_scanned.to_string().yellow(),
        duration.as_secs_f64() * 1000.0
    );
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
