use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod db;
mod download;
mod import;

#[derive(Parser)]
#[command(name = "data-loader")]
#[command(about = "Download and load market data into DuckDB", long_about = None)]
struct Cli {
    /// Database file path
    #[arg(short, long, default_value = "/home/molaco/.local/share/flowsurface/flowsurface.duckdb")]
    db_path: PathBuf,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download kline/candlestick data from Binance
    Klines {
        /// Trading pair symbol (e.g., BTCUSDT)
        #[arg(short, long)]
        ticker: String,

        /// Timeframe interval (e.g., 1m, 5m, 15m, 1h, 4h, 1d)
        #[arg(short, long)]
        interval: String,

        /// Start date (YYYY-MM-DD format)
        #[arg(short, long)]
        start: String,

        /// End date (YYYY-MM-DD format)
        #[arg(short, long)]
        end: String,

        /// Skip already downloaded data
        #[arg(long, default_value_t = true)]
        skip_existing: bool,
    },

    /// Download trade data from Binance
    Trades {
        /// Trading pair symbol (e.g., BTCUSDT)
        #[arg(short, long)]
        ticker: String,

        /// Start date (YYYY-MM-DD format)
        #[arg(short, long)]
        start: String,

        /// End date (YYYY-MM-DD format)
        #[arg(short, long)]
        end: String,

        /// Skip already downloaded data
        #[arg(long, default_value_t = true)]
        skip_existing: bool,
    },

    /// Import Binance ZIP archives from local directory
    Import {
        /// Path to directory containing ZIP files or single ZIP file
        #[arg(short, long)]
        path: PathBuf,

        /// Batch size for bulk inserts
        #[arg(short, long, default_value_t = 1000)]
        batch_size: usize,

        /// Dry run - don't actually write to database
        #[arg(long)]
        dry_run: bool,
    },

    /// Show database statistics
    Stats {
        /// Show detailed per-ticker statistics
        #[arg(short, long)]
        detailed: bool,

        /// Show stats for specific ticker (e.g., BTCUSDT)
        #[arg(short, long)]
        ticker: Option<String>,

        /// Show only trades statistics
        #[arg(long)]
        trades: bool,

        /// Show only klines statistics
        #[arg(long)]
        klines: bool,
    },

    /// Initialize database schema
    Init {
        /// Force re-initialization (WARNING: will drop existing data)
        #[arg(long)]
        force: bool,
    },

    /// Vacuum and optimize database
    Vacuum,

    /// Export data to CSV
    Export {
        /// Trading pair symbol
        #[arg(short, long)]
        ticker: String,

        /// Timeframe (for klines export)
        #[arg(short, long)]
        interval: Option<String>,

        /// Start date (YYYY-MM-DD format)
        #[arg(short, long)]
        start: String,

        /// End date (YYYY-MM-DD format)
        #[arg(short, long)]
        end: String,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Data type to export (trades or klines)
        #[arg(long, default_value = "trades")]
        data_type: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("Data Loader starting...");
    log::info!("Database: {}", cli.db_path.display());

    // Initialize database connection
    let mut db = db::DatabaseManager::new(&cli.db_path)?;
    log::info!("Database connection established");

    // Execute command
    match cli.command {
        Commands::Klines {
            ticker,
            interval,
            start,
            end,
            skip_existing,
        } => {
            use chrono::NaiveDate;
            use std::collections::HashSet;

            log::info!(
                "Downloading klines: {} {} from {} to {}",
                ticker,
                interval,
                start,
                end
            );
            log::info!("Skip existing: {}", skip_existing);

            // Parse dates
            let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("Invalid start date format: {}. Expected YYYY-MM-DD", e))?;
            let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("Invalid end date format: {}. Expected YYYY-MM-DD", e))?;

            // Validate date range
            if end_date < start_date {
                anyhow::bail!("End date must be after or equal to start date");
            }

            println!("\n=== Downloading Klines ===");
            println!("Ticker: {}", ticker);
            println!("Interval: {}", interval);
            println!("Date range: {} to {}", start_date, end_date);
            println!("Skip existing: {}\n", skip_existing);

            // Prepare download parameters
            let base_path = PathBuf::from("./binance-data");
            let market_type = download::binance::MarketKind::Spot;
            let concurrency = 3; // Download 3 files concurrently
            let skip_dates = if skip_existing {
                // TODO: Check which dates already exist in the database
                Some(HashSet::new())
            } else {
                None
            };

            // Download klines
            let result = download::binance::download_klines_multi_date(
                &ticker,
                market_type,
                &interval,
                start_date,
                end_date,
                base_path.clone(),
                concurrency,
                skip_dates,
                None, // No cancellation token for CLI
            ).await?;

            // Print summary
            println!("\n=== Download Summary ===");
            println!("Files downloaded: {}", result.stats.successful_files);
            println!("Files failed: {}", result.stats.failed_files);
            println!("Total records: {}", result.stats.total_records);
            println!("Total size: {:.2} MB", result.stats.total_bytes as f64 / 1_048_576.0);
            println!("Duration: {:.2}s", result.stats.duration.as_secs_f64());

            if !result.stats.errors.is_empty() {
                println!("\nErrors encountered: {}", result.stats.errors.len());
                for (i, err) in result.stats.errors.iter().enumerate() {
                    println!("  {}. {} - {}", i + 1, err.file_name, err.error);
                }
            }

            // Optionally auto-import the downloaded files
            if result.stats.successful_files > 0 {
                println!("\nWould you like to import the downloaded files now? (y/n)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes") {
                    println!("\n=== Importing Downloaded Files ===");

                    // Construct the download directory path
                    // Binance structure: binance-data/data/spot/daily/klines/[TICKER]/[INTERVAL]/
                    let download_dir = base_path
                        .join("data")
                        .join("spot")
                        .join("daily")
                        .join("klines")
                        .join(&ticker)
                        .join(&interval);

                    log::info!("Importing from: {}", download_dir.display());

                    if download_dir.exists() {
                        let importer = import::archive::ArchiveImporter::new_for_klines(1000, false);
                        let conn = db.get_connection_mut();
                        let import_stats = importer.import_zip_archives(conn, &download_dir)?;

                        println!("\n=== Import Summary ===");
                        println!("Files processed: {}", import_stats.files_processed);
                        println!("Klines imported: {}", import_stats.trades_migrated);

                        if !import_stats.errors.is_empty() {
                            println!("\nImport errors: {}", import_stats.errors.len());
                            for (i, err) in import_stats.errors.iter().enumerate() {
                                println!("  {}. {}", i + 1, err);
                            }
                        }
                    } else {
                        println!("Warning: Download directory not found at {}", download_dir.display());
                    }
                }
            }

            Ok(())
        }

        Commands::Trades {
            ticker,
            start,
            end,
            skip_existing,
        } => {
            use chrono::NaiveDate;
            use std::collections::HashSet;

            log::info!("Downloading trades: {} from {} to {}", ticker, start, end);
            log::info!("Skip existing: {}", skip_existing);

            // Parse dates
            let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("Invalid start date format: {}. Expected YYYY-MM-DD", e))?;
            let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
                .map_err(|e| anyhow::anyhow!("Invalid end date format: {}. Expected YYYY-MM-DD", e))?;

            // Validate date range
            if end_date < start_date {
                anyhow::bail!("End date must be after or equal to start date");
            }

            println!("\n=== Downloading Trades ===");
            println!("Ticker: {}", ticker);
            println!("Date range: {} to {}", start_date, end_date);
            println!("Skip existing: {}\n", skip_existing);

            // Prepare download parameters
            let base_path = PathBuf::from("./binance-data");
            let market_type = download::binance::MarketKind::Spot;
            let concurrency = 3; // Download 3 files concurrently
            let skip_dates = if skip_existing {
                // TODO: Check which dates already exist in the database
                Some(HashSet::new())
            } else {
                None
            };

            // Download trades
            let result = download::binance::download_trades_multi_date(
                &ticker,
                market_type,
                start_date,
                end_date,
                base_path.clone(),
                concurrency,
                skip_dates,
                None, // No cancellation token for CLI
            ).await?;

            // Print summary
            println!("\n=== Download Summary ===");
            println!("Files downloaded: {}", result.stats.successful_files);
            println!("Files failed: {}", result.stats.failed_files);
            println!("Total records: {}", result.stats.total_records);
            println!("Total size: {:.2} MB", result.stats.total_bytes as f64 / 1_048_576.0);
            println!("Duration: {:.2}s", result.stats.duration.as_secs_f64());

            if !result.stats.errors.is_empty() {
                println!("\nErrors encountered: {}", result.stats.errors.len());
                for (i, err) in result.stats.errors.iter().enumerate() {
                    println!("  {}. {} - {}", i + 1, err.file_name, err.error);
                }
            }

            // Optionally auto-import the downloaded files
            if result.stats.successful_files > 0 {
                println!("\nWould you like to import the downloaded files now? (y/n)");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes") {
                    println!("\n=== Importing Downloaded Files ===");

                    // Construct the download directory path
                    // Binance structure: binance-data/data/spot/daily/aggTrades/[TICKER]/
                    let download_dir = base_path
                        .join("data")
                        .join("spot")
                        .join("daily")
                        .join("aggTrades")
                        .join(&ticker);

                    log::info!("Importing from: {}", download_dir.display());

                    if download_dir.exists() {
                        let importer = import::archive::ArchiveImporter::new_for_trades(1000, false);
                        let conn = db.get_connection_mut();
                        let import_stats = importer.import_zip_archives(conn, &download_dir)?;

                        println!("\n=== Import Summary ===");
                        println!("Files processed: {}", import_stats.files_processed);
                        println!("Trades imported: {}", import_stats.trades_migrated);

                        if !import_stats.errors.is_empty() {
                            println!("\nImport errors: {}", import_stats.errors.len());
                            for (i, err) in import_stats.errors.iter().enumerate() {
                                println!("  {}. {}", i + 1, err);
                            }
                        }
                    } else {
                        println!("Warning: Download directory not found at {}", download_dir.display());
                    }
                }
            }

            Ok(())
        }

        Commands::Import {
            path,
            batch_size,
            dry_run,
        } => {
            log::info!("Importing archives from: {}", path.display());
            log::info!("Batch size: {}, Dry run: {}", batch_size, dry_run);

            // Default to trades for backward compatibility
            // TODO: Add a flag to specify data type (trades vs klines)
            let importer = import::archive::ArchiveImporter::new_for_trades(batch_size, dry_run);
            let conn = db.get_connection_mut();
            let stats = importer.import_zip_archives(conn, &path)?;

            println!("\n=== Import Summary ===");
            println!("Files processed: {}", stats.files_processed);
            println!("Trades imported: {}", stats.trades_migrated);

            if !stats.errors.is_empty() {
                println!("\nErrors encountered: {}", stats.errors.len());
                for (i, err) in stats.errors.iter().enumerate() {
                    println!("  {}. {}", i + 1, err);
                }
            }

            Ok(())
        }

        Commands::Stats { detailed, ticker, trades, klines } => {
            log::info!("Fetching database statistics...");

            // If specific ticker requested
            if let Some(ticker_symbol) = ticker {
                match db.get_ticker_stats(&ticker_symbol)? {
                    Some(ticker_stats) => {
                        println!("\n=== Ticker: {} ({}) ===", ticker_stats.symbol, ticker_stats.exchange);

                        // Show trades if not filtered to klines-only
                        if !klines {
                            if ticker_stats.trade_count > 0 {
                                print!("\nTrades: {}", ticker_stats.trade_count);
                                if let (Some(start), Some(end)) = (&ticker_stats.trade_start_date, &ticker_stats.trade_end_date) {
                                    print!(" ({} to {})", start, end);
                                }
                                println!();
                            } else {
                                println!("\nNo trades data");
                            }
                        }

                        // Show klines if not filtered to trades-only
                        if !trades {
                            if !ticker_stats.kline_intervals.is_empty() {
                                println!("\nKlines:");
                                for interval in &ticker_stats.kline_intervals {
                                    print!("  {}: {} candles", interval.interval, interval.count);
                                    if let (Some(start), Some(end)) = (&interval.start_date, &interval.end_date) {
                                        print!(" ({} to {})", start, end);
                                    }
                                    println!();
                                }
                            } else {
                                println!("\nNo klines data");
                            }
                        }
                    }
                    None => {
                        println!("Ticker '{}' not found in database", ticker_symbol);
                    }
                }
            } else {
                // Show overall database stats
                let stats = db.get_stats()?;

                println!("\n=== Database Statistics ===");
                println!("Total trades:  {}", stats.total_trades);
                println!("Total klines:  {}", stats.total_klines);
                println!("Total tickers: {}", stats.total_tickers);
                println!(
                    "Database size: {:.2} MB",
                    stats.database_size_bytes as f64 / 1_048_576.0
                );
                println!("Schema version: {}", stats.schema_version);

                // Show per-ticker summary if detailed
                if detailed {
                    log::info!("Fetching detailed per-ticker statistics...");
                    let ticker_stats = db.get_all_ticker_stats()?;

                    if !ticker_stats.is_empty() {
                        println!("\n=== Per-Ticker Summary ===");

                        for ticker in &ticker_stats {
                            println!("\n{} ({}):", ticker.symbol, ticker.exchange);

                            // Show trades if not filtered to klines-only
                            if !klines && ticker.trade_count > 0 {
                                print!("  Trades: {}", ticker.trade_count);
                                if let (Some(start), Some(end)) = (&ticker.trade_start_date, &ticker.trade_end_date) {
                                    print!(" ({} to {})", start, end);
                                }
                                println!();
                            }

                            // Show klines if not filtered to trades-only
                            if !trades && !ticker.kline_intervals.is_empty() {
                                println!("  Klines:");
                                for interval in &ticker.kline_intervals {
                                    print!("    {}: {} candles", interval.interval, interval.count);
                                    if let (Some(start), Some(end)) = (&interval.start_date, &interval.end_date) {
                                        print!(" ({} to {})", start, end);
                                    }
                                    println!();
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        }

        Commands::Init { force } => {
            if force {
                log::warn!("Force initialization requested - this will drop existing data!");
                println!("Are you sure? This will delete all existing data. Type 'yes' to confirm:");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if input.trim() != "yes" {
                    log::info!("Initialization cancelled");
                    return Ok(());
                }
            }
            log::info!("Initializing database schema...");
            // Schema initialization happens in DatabaseManager::new()
            log::info!("Database initialized successfully");
            Ok(())
        }

        Commands::Vacuum => {
            log::info!("Running VACUUM and ANALYZE...");
            db.vacuum()?;
            log::info!("Database optimized successfully");
            Ok(())
        }

        Commands::Export {
            ticker,
            interval,
            start,
            end,
            output,
            data_type,
        } => {
            log::info!(
                "Exporting {} data for {} from {} to {} to {}",
                data_type,
                ticker,
                start,
                end,
                output.display()
            );
            if let Some(iv) = interval {
                log::info!("Interval: {}", iv);
            }
            // TODO: Implement data export
            log::warn!("Data export not yet implemented");
            Ok(())
        }
    }
}
